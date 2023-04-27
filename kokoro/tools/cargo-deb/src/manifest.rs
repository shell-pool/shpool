use crate::config::CargoConfig;
use crate::dependencies::resolve;
use crate::{dh_installsystemd, debian_architecture_from_rust_triple};
use crate::error::{CDResult, CargoDebError};
use crate::listener::Listener;
use crate::ok_or::OkOrThen;
use crate::pathbytes::AsUnixPathBytes;
use crate::util::read_file_to_bytes;
use cargo_toml::DebugSetting;
use cargo_toml::OptionalFile;
use rayon::prelude::*;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::env::consts::EXE_SUFFIX;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn is_glob_pattern(s: &Path) -> bool {
    s.to_bytes().iter().any(|&c| c == b'*' || c == b'[' || c == b']' || c == b'!')
}

#[derive(Debug, Clone)]
pub enum AssetSource {
    /// Copy file from the path (and strip binary if needed).
    Path(PathBuf),
    /// A symlink existing in the file system
    Symlink(PathBuf),
    /// Write data to destination as-is.
    Data(Vec<u8>),
}

impl AssetSource {
    /// Symlink must exist on disk to be preserved
    #[must_use]
    pub fn from_path(path: impl Into<PathBuf>, preserve_existing_symlink: bool) -> Self {
        let path = path.into();
        if preserve_existing_symlink || !path.exists() { // !exists means a symlink to bogus path
            if let Ok(md) = fs::symlink_metadata(&path) {
                if md.is_symlink() {
                    return Self::Symlink(path)
                }
            }
        }
        Self::Path(path)
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            AssetSource::Symlink(ref p) |
            AssetSource::Path(ref p) => Some(p),
            _ => None,
        }
    }

    pub fn archive_as_symlink_only(&self) -> bool {
        matches!(self, AssetSource::Symlink(_))
    }

    #[must_use]
    pub fn file_size(&self) -> Option<u64> {
        match *self {
            // FIXME: may not be accurate if the executable is not stripped yet?
            AssetSource::Path(ref p) => fs::metadata(p).ok().map(|m| m.len()),
            AssetSource::Data(ref d) => Some(d.len() as u64),
            AssetSource::Symlink(_) => None,
        }
    }

    pub fn data(&self) -> CDResult<Cow<'_, [u8]>> {
        Ok(match self {
            AssetSource::Path(p) => {
                let data = read_file_to_bytes(p)
                    .map_err(|e| CargoDebError::IoFile("unable to read asset to add to archive", e, p.to_owned()))?;
                Cow::Owned(data)
            },
            AssetSource::Data(d) => Cow::Borrowed(d),
            AssetSource::Symlink(_) => return Err(CargoDebError::Str("Symlink unexpectedly used to read file data")),
        })
    }

    /// Return the file that will hold debug symbols for this asset.
    /// This is just `<original-file>.debug`
    #[must_use]
    pub fn debug_source(&self) -> Option<PathBuf> {
        match self {
            AssetSource::Path(p) |
            AssetSource::Symlink(p) => Some(debug_filename(p)),
            _ => None,
        }
    }
}

/// Configuration settings for the systemd_units functionality.
///
/// `unit_scripts`: (optional) relative path to a directory containing correctly
/// named systemd unit files. See `dh_lib::pkgfile()` and `dh_installsystemd.rs`
/// for more details on file naming. If not supplied, defaults to the
/// `maintainer_scripts` directory.
///
/// `unit_name`: (optjonal) in cases where the `unit_scripts` directory contains
/// multiple units, only process those matching this unit name.
///
/// For details on the other options please see `dh_installsystemd::Options`.
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct SystemdUnitsConfig {
    pub unit_scripts: Option<PathBuf>,
    pub unit_name: Option<String>,
    pub enable: Option<bool>,
    pub start: Option<bool>,
    pub restart_after_upgrade: Option<bool>,
    pub stop_on_upgrade: Option<bool>,
}

/// Match the official dh_installsystemd defaults and rename the confusing
/// dh_installsystemd option names to be consistently positive rather than
/// mostly, but not always, negative.
impl From<&SystemdUnitsConfig> for dh_installsystemd::Options {
    fn from(config: &SystemdUnitsConfig) -> Self {
        Self {
            no_enable: !config.enable.unwrap_or(true),
            no_start: !config.start.unwrap_or(true),
            restart_after_upgrade: config.restart_after_upgrade.unwrap_or(true),
            no_stop_on_upgrade: !config.stop_on_upgrade.unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Assets {
    pub unresolved: Vec<UnresolvedAsset>,
    pub resolved: Vec<Asset>,
}

impl Assets {
    fn new() -> Assets {
        Assets {
            unresolved: vec![],
            resolved: vec![],
        }
    }

    fn with_resolved_assets(assets: Vec<Asset>) -> Assets {
        Assets {
            unresolved: vec![],
            resolved: assets,
        }
    }

    fn with_unresolved_assets(assets: Vec<UnresolvedAsset>) -> Assets {
        Assets {
            unresolved: assets,
            resolved: vec![],
        }
    }

    fn is_empty(&self) -> bool {
        self.unresolved.is_empty() && self.resolved.is_empty()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IsBuilt {
    No,
    SamePackage,
    /// needs --workspace to build
    Workspace,
}

#[derive(Debug, Clone)]
pub struct UnresolvedAsset {
    pub source_path: PathBuf,
    pub c: AssetCommon,
}

#[derive(Debug, Clone)]
pub struct AssetCommon {
    pub target_path: PathBuf,
    pub chmod: u32,
    is_built: IsBuilt,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub source: AssetSource,
    pub c: AssetCommon,
}

impl Asset {
    #[must_use]
    pub fn new(source: AssetSource, mut target_path: PathBuf, chmod: u32, is_built: IsBuilt) -> Self {
        // is_dir() is only for paths that exist
        if target_path.to_string_lossy().ends_with('/') {
            let file_name = source.path().and_then(|p| p.file_name()).expect("source must be a file");
            target_path = target_path.join(file_name);
        }

        if target_path.is_absolute() || target_path.has_root() {
            target_path = target_path.strip_prefix("/").expect("no root dir").to_owned();
        }

        Self {
            source,
            c: AssetCommon {
                target_path, chmod, is_built,
            },
        }
    }
}

impl AssetCommon {
    fn is_executable(&self) -> bool {
        0 != self.chmod & 0o111
    }

    fn is_dynamic_library(&self) -> bool {
        is_dynamic_library_filename(&self.target_path)
    }

    /// Returns the target path for the debug symbol file, which will be
    /// /usr/lib/debug/<path-to-executable>.debug
    #[must_use]
    pub fn debug_target(&self) -> Option<PathBuf> {
        if self.is_built != IsBuilt::No {
            // Turn an absolute path into one relative to "/"
            let relative = match self.target_path.strip_prefix(Path::new("/")) {
                Ok(path) => path,
                Err(_) => self.target_path.as_path(),
            };

            // Prepend the debug location
            let debug_path = Path::new("/usr/lib/debug").join(relative);

            // Add `.debug` to the end of the filename
            Some(debug_filename(&debug_path))
        } else {
            None
        }
    }
}

/// Adds `.debug` to the end of a path to a filename
///
fn debug_filename(path: &Path) -> PathBuf {
    let mut debug_filename = path.as_os_str().to_os_string();
    debug_filename.push(".debug");
    Path::new(&debug_filename).to_path_buf()
}

fn is_dynamic_library_filename(path: &Path) -> bool {
    path.file_name()
        .and_then(|f| f.to_str())
        .map_or(false, |f| f.ends_with(DLL_SUFFIX))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ArchSpec {
    /// e.g. [armhf]
    Require(String),
    /// e.g. [!armhf]
    NegRequire(String),
}

fn get_architecture_specification(depend: &str) -> CDResult<(String, Option<ArchSpec>)> {
    use ArchSpec::*;
    let re = regex::Regex::new(r#"(.*)\[(!?)(.*)\]"#).unwrap();
    match re.captures(depend) {
        Some(caps) => {
            let spec = if &caps[2] == "!" {
                NegRequire(caps[3].to_string())
            } else {
                assert_eq!(&caps[2], "");
                Require(caps[3].to_string())
            };
            Ok((caps[1].trim().to_string(), Some(spec)))
        }
        None => Ok((depend.to_string(), None)),
    }
}

/// Architecture specification strings
/// <https://www.debian.org/doc/debian-policy/ch-customized-programs.html#s-arch-spec>
fn match_architecture(spec: ArchSpec, target_arch: &str) -> CDResult<bool> {
    let (neg, spec) = match spec {
        ArchSpec::NegRequire(pkg) => (true, pkg),
        ArchSpec::Require(pkg) => (false, pkg),
    };
    let output = Command::new("dpkg-architecture")
        .args(["-a", target_arch, "-i", &spec])
        .output()
        .map_err(|e| CargoDebError::CommandFailed(e, "dpkg-architecture"))?;
    if neg {
        Ok(!output.status.success())
    } else {
        Ok(output.status.success())
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Cargo deb configuration read from the manifest and cargo metadata
pub struct Config {
    /// Directory where `Cargo.toml` is located. It's a subdirectory in workspaces.
    pub package_manifest_dir: PathBuf,
    /// User-configured output path for *.deb
    pub deb_output_path: Option<String>,
    /// Triple. `None` means current machine architecture.
    pub target: Option<String>,
    /// `CARGO_TARGET_DIR`
    pub target_dir: PathBuf,
    /// The name of the project to build
    pub name: String,
    /// The name to give the Debian package; usually the same as the Cargo project name
    pub deb_name: String,
    /// The version to give the Debian package; usually the same as the Cargo version
    pub deb_version: String,
    /// The software license of the project (SPDX format).
    pub license: Option<String>,
    /// The location of the license file
    pub license_file: Option<PathBuf>,
    /// number of lines to skip when reading `license_file`
    pub license_file_skip_lines: usize,
    /// The copyright of the project
    /// (Debian's `copyright` file contents).
    pub copyright: String,
    pub changelog: Option<String>,
    /// The homepage URL of the project.
    pub homepage: Option<String>,
    /// Documentation URL from `Cargo.toml`. Fallback if `homepage` is missing.
    pub documentation: Option<String>,
    /// The URL of the software repository.
    pub repository: Option<String>,
    /// A short description of the project.
    pub description: String,
    /// An extended description of the project.
    pub extended_description: Option<String>,
    /// The maintainer of the Debian package.
    /// In Debian `control` file `Maintainer` field format.
    pub maintainer: String,
    /// The Debian dependencies required to run the project.
    pub depends: String,
    /// The Debian pre-dependencies.
    pub pre_depends: Option<String>,
    /// The Debian recommended dependencies.
    pub recommends: Option<String>,
    /// The Debian suggested dependencies.
    pub suggests: Option<String>,
    /// The list of packages this package can enhance.
    pub enhances: Option<String>,
    /// The Debian software category to which the package belongs.
    pub section: Option<String>,
    /// The Debian priority of the project. Typically 'optional'.
    pub priority: String,

    /// `Conflicts` Debian control field.
    ///
    /// See [PackageTransition](https://wiki.debian.org/PackageTransition).
    pub conflicts: Option<String>,
    /// `Breaks` Debian control field.
    ///
    /// See [PackageTransition](https://wiki.debian.org/PackageTransition).
    pub breaks: Option<String>,
    /// `Replaces` Debian control field.
    ///
    /// See [PackageTransition](https://wiki.debian.org/PackageTransition).
    pub replaces: Option<String>,
    /// `Provides` Debian control field.
    ///
    /// See [PackageTransition](https://wiki.debian.org/PackageTransition).
    pub provides: Option<String>,

    /// The Debian architecture of the target system.
    pub architecture: String,
    /// A list of configuration files installed by the package.
    pub conf_files: Option<String>,
    /// All of the files that are to be packaged.
    pub(crate) assets: Assets,
    /// The location of the triggers file
    pub triggers_file: Option<PathBuf>,
    /// The path where possible maintainer scripts live
    pub maintainer_scripts: Option<PathBuf>,
    /// List of Cargo features to use during build
    pub features: Vec<String>,
    pub default_features: bool,
    /// Should the binary be stripped from debug symbols?
    pub debug_enabled: bool,
    /// Should the debug symbols be moved to a separate file included in the package? (implies `strip:true`)
    pub separate_debug_symbols: bool,
    /// Should symlinks be preserved in the assets
    pub preserve_symlinks: bool,
    /// Details of how to install any systemd units
    pub(crate) systemd_units: Option<Vec<SystemdUnitsConfig>>,

    /// unix timestamp for generated files
    pub default_timestamp: u64,
}

impl Config {
    /// Makes a new config from `Cargo.toml` in the `manifest_path`
    ///
    /// `None` target means the host machine's architecture.
    pub fn from_manifest(manifest_path: &Path, selected_package_name: Option<&str>, output_path: Option<String>, target: Option<&str>, variant: Option<&str>, deb_version: Option<String>, deb_revision: Option<String>, listener: &dyn Listener, selected_profile: &str) -> CDResult<Config> {
        let metadata = cargo_metadata(manifest_path)?;
        let available_package_names = || {
            metadata.packages.iter()
                .filter(|p| metadata.workspace_members.iter().any(|w| w == &p.id))
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>().join(", ")
        };
        let target_package = if let Some(name) = selected_package_name {
            metadata.packages.iter().find(|p| p.name == name)
                .ok_or_else(|| CargoDebError::PackageNotFoundInWorkspace(name.into(), available_package_names()))
        } else {
            metadata.resolve.root.as_ref().and_then(|root_id| {
                metadata.packages.iter()
                    .find(move |p| &p.id == root_id)
            })
            .ok_or_else(|| CargoDebError::NoRootFoundInWorkspace(available_package_names()))
        }?;
        let workspace_root_manifest_path = Path::new(&metadata.workspace_root).join("Cargo.toml");
        let workspace_root_manifest = cargo_toml::Manifest::<CargoPackageMetadata>::from_path_with_metadata(&workspace_root_manifest_path).ok();

        let target_dir = Path::new(&metadata.target_directory);
        let manifest_path = Path::new(&target_package.manifest_path);
        let package_manifest_dir = manifest_path.parent().unwrap();
        let manifest_bytes =
            fs::read(manifest_path).map_err(|e| CargoDebError::IoFile("unable to read manifest", e, manifest_path.to_owned()))?;
        let manifest_mdate = std::fs::metadata(manifest_path)?.modified().unwrap_or_else(|_| SystemTime::now());
        let default_timestamp = manifest_mdate.duration_since(SystemTime::UNIX_EPOCH).expect("bad clock").as_secs();

        let mut manifest = cargo_toml::Manifest::<CargoPackageMetadata>::from_slice_with_metadata(&manifest_bytes)
            .map_err(|e| CargoDebError::TomlParsing(e, manifest_path.into()))?;
        let ws_root = workspace_root_manifest.as_ref().map(|ws| (ws, Path::new(&metadata.workspace_root)));
        manifest.complete_from_path_and_workspace(manifest_path, ws_root)
            .map_err(move |e| CargoDebError::TomlParsing(e, manifest_path.to_path_buf()))?;
        Self::from_manifest_inner(manifest, workspace_root_manifest.as_ref(), target_package, package_manifest_dir, output_path, target_dir, target, variant, deb_version, deb_revision, listener, selected_profile, default_timestamp)
    }

    /// Convert Cargo.toml/metadata information into internal config structure
    ///
    /// **IMPORTANT**: This function must not create or expect to see any files on disk!
    /// It's run before destination directory is cleaned up, and before the build start!
    fn from_manifest_inner(
        mut manifest: cargo_toml::Manifest<CargoPackageMetadata>,
        root_manifest: Option<&cargo_toml::Manifest<CargoPackageMetadata>>,
        cargo_metadata: &CargoMetadataPackage,
        package_manifest_dir: &Path,
        deb_output_path: Option<String>,
        target_dir: &Path,
        target: Option<&str>,
        variant: Option<&str>,
        deb_version: Option<String>,
        deb_revision: Option<String>,
        listener: &dyn Listener,
        selected_profile: &str,
        default_timestamp: u64,
    ) -> CDResult<Self> {
        // Cargo cross-compiles to a dir
        let target_dir = if let Some(target) = target {
            target_dir.join(target)
        } else {
            target_dir.to_owned()
        };

        // FIXME: support other named profiles
        let debug_enabled = if selected_profile == "release" {
            debug_flag(&manifest) || root_manifest.map_or(false, debug_flag)
        } else {
            false
        };
        let package = manifest.package.as_mut().unwrap();

        // If we build against a variant use that config and change the package name
        let mut deb = if let Some(variant) = variant {
            // Use dash as underscore is not allowed in package names
            package.name = format!("{}-{variant}", package.name);
            let mut deb = package.metadata.take()
                .and_then(|m| m.deb).unwrap_or_default();
            let variant = deb.variants
                .as_mut()
                .and_then(|v| v.remove(variant))
                .ok_or_else(|| CargoDebError::VariantNotFound(variant.to_string()))?;
            variant.inherit_from(deb)
        } else {
            package.metadata.take().and_then(|m| m.deb).unwrap_or_default()
        };

        let (license_file, license_file_skip_lines) = manifest_license_file(package, deb.license_file.as_ref())?;

        manifest_check_config(package, package_manifest_dir, &deb, listener);
        let extended_description = manifest_extended_description(
            deb.extended_description.take(),
            deb.extended_description_file.as_ref().map(Path::new).or(package.readme().as_path()),
        )?;
        let mut config = Config {
            default_timestamp,
            package_manifest_dir: package_manifest_dir.to_owned(),
            deb_output_path,
            target: target.map(|t| t.to_string()),
            target_dir,
            name: package.name.clone(),
            deb_name: deb.name.take().unwrap_or_else(|| debian_package_name(&package.name)),
            deb_version: deb_version.unwrap_or_else(|| manifest_version_string(package, deb_revision.or(deb.revision))),
            license: package.license.take().map(|v| v.unwrap()),
            license_file,
            license_file_skip_lines,
            copyright: deb.copyright.take().ok_or_then(|| {
                if package.authors().is_empty() {
                    return Err("The package must have a copyright or authors property".into());
                }
                Ok(package.authors().join(", "))
            })?,
            homepage: package.homepage().map(From::from),
            documentation: package.documentation().map(From::from),
            repository: package.repository.take().map(|v| v.unwrap()),
            description: package.description.take().map(|v| v.unwrap()).unwrap_or_else(||format!("[generated from Rust crate {}]", package.name)),
            extended_description,
            maintainer: deb.maintainer.take().ok_or_then(|| {
                Ok(package.authors().get(0)
                    .ok_or("The package must have a maintainer or authors property")?.to_owned())
            })?,
            depends: deb.depends.take().unwrap_or_else(|| "$auto".to_owned()),
            pre_depends: deb.pre_depends.take(),
            recommends: deb.recommends.take(),
            suggests: deb.suggests.take(),
            enhances: deb.enhances.take(),
            conflicts: deb.conflicts.take(),
            breaks: deb.breaks.take(),
            replaces: deb.replaces.take(),
            provides: deb.provides.take(),
            section: deb.section.take(),
            priority: deb.priority.take().unwrap_or_else(|| "optional".to_owned()),
            architecture: debian_architecture_from_rust_triple(target.unwrap_or(crate::DEFAULT_TARGET)).to_owned(),
            conf_files: deb.conf_files.map(|x| format_conffiles(&x)),
            assets: Assets::new(),
            triggers_file: deb.triggers_file.map(PathBuf::from),
            changelog: deb.changelog.take(),
            maintainer_scripts: deb.maintainer_scripts.map(PathBuf::from),
            features: deb.features.take().unwrap_or_default(),
            default_features: deb.default_features.unwrap_or(true),
            separate_debug_symbols: deb.separate_debug_symbols.unwrap_or(false),
            debug_enabled,
            preserve_symlinks: deb.preserve_symlinks.unwrap_or(false),
            systemd_units: match deb.systemd_units {
                None => None,
                Some(SystemUnitsSingleOrMultiple::Single(s)) => { Some(vec![s]) }
                Some(SystemUnitsSingleOrMultiple::Multi(v)) => { Some(v) }
            },
        };
        config.take_assets(package, deb.assets.take(), &cargo_metadata.targets, selected_profile, listener)?;
        config.add_copyright_asset()?;
        config.add_changelog_asset()?;
        config.add_systemd_assets()?;

        Ok(config)
    }

    pub(crate) fn get_dependencies(&self, listener: &dyn Listener) -> CDResult<String> {
        let mut deps = HashSet::new();
        for word in self.depends.split(',') {
            let word = word.trim();
            if word == "$auto" {
                let bin = self.all_binaries();
                let resolved = bin.par_iter()
                    .filter(|bin| !bin.archive_as_symlink_only())
                    .filter_map(|p| p.path())
                    .filter_map(|bname| match resolve(bname, &self.target) {
                        Ok(bindeps) => Some(bindeps),
                        Err(err) => {
                            listener.warning(format!("{} (no auto deps for {})", err, bname.display()));
                            None
                        },
                    })
                    .collect::<Vec<_>>();
                for dep in resolved.into_iter().flat_map(|s| s.into_iter()) {
                    deps.insert(dep);
                }
            } else {
                let (dep, arch_spec) = get_architecture_specification(word)?;
                if let Some(spec) = arch_spec {
                    if match_architecture(spec, &self.architecture)? {
                        deps.insert(dep);
                    }
                } else {
                    deps.insert(dep);
                }
            }
        }
        Ok(deps.into_iter().collect::<Vec<_>>().join(", "))
    }

    pub fn extend_cargo_build_flags(&self, flags: &mut Vec<String>) {
        if flags.iter().any(|f| f == "--workspace" || f == "--all") {
            return;
        }

        for a in self.assets.unresolved.iter().filter(|a| a.c.is_built != IsBuilt::No) {
            if is_glob_pattern(&a.source_path) {
                log::debug!("building entire workspace because of glob {}", a.source_path.display());
                flags.push("--workspace".into());
                return;
            }
        }

        let mut build_bins = vec![];
        let mut build_libs = false;
        let mut same_package = true;
        let resolved = self.assets.resolved.iter().map(|a| (&a.c, a.source.path()));
        let unresolved = self.assets.unresolved.iter().map(|a| (&a.c, Some(a.source_path.as_ref())));
        for (asset_target, source_path) in resolved.chain(unresolved).filter(|(c,_)| c.is_built != IsBuilt::No) {
            if asset_target.is_built != IsBuilt::SamePackage {
                log::debug!("building workspace because {} is from another package", source_path.unwrap_or(&asset_target.target_path).display());
                same_package = false;
            }
            if asset_target.is_dynamic_library() || source_path.map_or(false, is_dynamic_library_filename) {
                log::debug!("building libs for {}", source_path.unwrap_or(&asset_target.target_path).display());
                build_libs = true;
            } else if asset_target.is_executable() {
                if let Some(source_path) = source_path {
                    let name = source_path.file_name().unwrap().to_str().expect("utf-8 target name");
                    let name = name.strip_suffix(EXE_SUFFIX).unwrap_or(name);
                    build_bins.push(name);
                }
            }
        }

        if !same_package {
            flags.push("--workspace".into());
        }
        flags.extend(build_bins.iter().map(|name| {
            log::debug!("building bin for {}", name);
            format!("--bin={name}")
        }));
        if build_libs {
            flags.push("--lib".into());
        }
    }

    pub fn resolve_assets(&mut self) -> CDResult<()> {
        for UnresolvedAsset { source_path, c: AssetCommon { target_path, chmod, is_built } } in self.assets.unresolved.drain(..) {
            let source_prefix: PathBuf = source_path.iter()
                .take_while(|part| !is_glob_pattern(part.as_ref()))
                .collect();
            let source_is_glob = is_glob_pattern(&source_path);
            let file_matches = glob::glob(source_path.to_str().expect("utf8 path"))?
                // Remove dirs from globs without throwing away errors
                .map(|entry| {
                    let source_file = entry?;
                    Ok(if source_file.is_dir() { None } else { Some(source_file) })
                })
                .filter_map(|res| match res {
                    Ok(None) => None,
                    Ok(Some(x)) => Some(Ok(x)),
                    Err(x) => Some(Err(x)),
                })
                .collect::<CDResult<Vec<_>>>()?;

            // If glob didn't match anything, it's likely an error
            // as all files should exist when called to resolve
            if file_matches.is_empty() {
                return Err(CargoDebError::AssetFileNotFound(source_path));
            }

            for source_file in file_matches {
                // XXX: how do we handle duplicated assets?
                let target_file = if source_is_glob {
                    target_path.join(source_file.strip_prefix(&source_prefix).unwrap())
                } else {
                    target_path.clone()
                };
                log::debug!("asset {} -> {} {} {:o}", source_file.display(), target_file.display(), if is_built == IsBuilt::No {"copy"} else {"build"}, chmod);
                self.assets.resolved.push(Asset::new(
                    AssetSource::from_path(source_file, self.preserve_symlinks),
                    target_file,
                    chmod,
                    is_built,
                ));
            }
        }

        self.sort_assets_by_type();
        Ok(())
    }

    pub(crate) fn add_copyright_asset(&mut self) -> CDResult<()> {
        let copyright_file = crate::data::generate_copyright_asset(self)?;
        log::debug!("added copyright");
        self.assets.resolved.push(Asset::new(
            AssetSource::Data(copyright_file),
            Path::new("usr/share/doc").join(&self.deb_name).join("copyright"),
            0o644,
            IsBuilt::No,
        ));
        Ok(())
    }

    pub fn add_debug_assets(&mut self) {
        let mut assets_to_add: Vec<Asset> = Vec::new();
        for asset in self.built_binaries_mut().into_iter().filter(|a| a.source.path().is_some()) {
            let debug_source = asset.source.debug_source().expect("debug asset");
            if debug_source.exists() {
                log::debug!("added debug file {}", debug_source.display());
                let debug_target = asset.c.debug_target().expect("debug asset");
                assets_to_add.push(Asset::new(
                    AssetSource::Path(debug_source),
                    debug_target,
                    0o644,
                    IsBuilt::No,
                ));
            } else {
                log::debug!("no debug file {}", debug_source.display());
            }
        }
        self.assets.resolved.append(&mut assets_to_add);
    }

    fn add_changelog_asset(&mut self) -> CDResult<()> {
        // The file is autogenerated later
        if self.changelog.is_some() {
            if let Some(changelog_file) = crate::data::generate_changelog_asset(self)? {
                log::debug!("added changelog");
                self.assets.resolved.push(Asset::new(
                    AssetSource::Data(changelog_file),
                    Path::new("usr/share/doc").join(&self.deb_name).join("changelog.Debian.gz"),
                    0o644,
                    IsBuilt::No,
                ));
            }
        }
        Ok(())
    }

    fn add_systemd_assets(&mut self) -> CDResult<()> {
        if let Some(ref config_vec) = self.systemd_units {
            for config in config_vec {
                let units_dir_option = config.unit_scripts.as_ref()
                    .or(self.maintainer_scripts.as_ref());
                if let Some(unit_dir) = units_dir_option {
                    let search_path = self.path_in_package(unit_dir);
                    let package = &self.name;
                    let unit_name = config.unit_name.as_deref();

                    let units = dh_installsystemd::find_units(&search_path, package, unit_name);

                    for (source, target) in units {
                        self.assets.resolved.push(Asset::new(
                            AssetSource::from_path(source, self.preserve_symlinks), // should this even support symlinks at all?
                            target.path,
                            target.mode,
                            IsBuilt::No,
                        ));
                    }
                }
            }
        } else {
            log::debug!("no systemd units to generate");
        }
        Ok(())
    }

    /// Executables AND dynamic libraries. May include symlinks.
    fn all_binaries(&self) -> Vec<&AssetSource> {
        self.assets.resolved.iter()
            .filter(|asset| {
                // Assumes files in build dir which have executable flag set are binaries
                asset.c.is_dynamic_library() || asset.c.is_executable()
            })
            .map(|asset| &asset.source)
            .collect()
    }

    /// Executables AND dynamic libraries, but only in `target/release`
    pub(crate) fn built_binaries_mut(&mut self) -> Vec<&mut Asset> {
        self.assets.resolved.iter_mut()
            .filter(move |asset| {
                // Assumes files in build dir which have executable flag set are binaries
                asset.c.is_built != IsBuilt::No && (asset.c.is_dynamic_library() || asset.c.is_executable())
            })
            .collect()
    }

    /// Tries to guess type of source control used for the repo URL.
    /// It's a guess, and it won't be 100% accurate, because Cargo suggests using
    /// user-friendly URLs or webpages instead of tool-specific URL schemes.
    pub(crate) fn repository_type(&self) -> Option<&str> {
        if let Some(ref repo) = self.repository {
            if repo.starts_with("git+")
                || repo.ends_with(".git")
                || repo.contains("git@")
                || repo.contains("github.com")
                || repo.contains("gitlab.com")
            {
                return Some("Git");
            }
            if repo.starts_with("cvs+") || repo.contains("pserver:") || repo.contains("@cvs.") {
                return Some("Cvs");
            }
            if repo.starts_with("hg+") || repo.contains("hg@") || repo.contains("/hg.") {
                return Some("Hg");
            }
            if repo.starts_with("svn+") || repo.contains("/svn.") {
                return Some("Svn");
            }
            return None;
        }
        None
    }

    pub(crate) fn path_in_build<P: AsRef<Path>>(&self, rel_path: P, profile: &str) -> PathBuf {
        let profile = match profile {
            "dev" => "debug",
            p => p,
        };

        let mut path = self.target_dir.join(profile);
        path.push(rel_path);
        path
    }

    pub(crate) fn path_in_package<P: AsRef<Path>>(&self, rel_path: P) -> PathBuf {
        self.package_manifest_dir.join(rel_path)
    }

    /// Store intermediate files here
    pub(crate) fn deb_temp_dir(&self) -> PathBuf {
        self.target_dir.join("debian").join(&self.name)
    }

    /// Save final .deb here
    pub(crate) fn deb_output_path(&self, filename: &str) -> PathBuf {
        if let Some(ref path_str) = self.deb_output_path {
            let path = Path::new(path_str);
            if path_str.ends_with('/') || path.is_dir() {
                path.join(filename)
            } else {
                path.to_owned()
            }
        } else {
            self.default_deb_output_dir().join(filename)
        }
    }

    pub(crate) fn default_deb_output_dir(&self) -> PathBuf {
        self.target_dir.join("debian")
    }

    pub(crate) fn cargo_config(&self) -> CDResult<Option<CargoConfig>> {
        CargoConfig::new(&self.target_dir)
    }

    /// similar files next to each other improve tarball compression
    pub(crate) fn sort_assets_by_type(&mut self) {
        self.assets.resolved.sort_by(|a,b| {
            a.c.is_executable().cmp(&b.c.is_executable())
            .then(a.c.is_dynamic_library().cmp(&b.c.is_dynamic_library()))
            .then(a.c.target_path.extension().cmp(&b.c.target_path.extension()))
            .then(a.c.target_path.parent().cmp(&b.c.target_path.parent()))
        });
    }
}

/// Debian doesn't like `_` in names
fn debian_package_name(crate_name: &str) -> String {
    // crate names are ASCII only
    crate_name.bytes().map(|c| {
        if c != b'_' {c.to_ascii_lowercase() as char} else {'-'}
    }).collect()
}

fn debug_flag(manifest: &cargo_toml::Manifest<CargoPackageMetadata>) -> bool {
    manifest.profile.release.as_ref()
        .and_then(|r| r.debug.as_ref())
        .map_or(false, |debug| match debug {
            DebugSetting::None => false,
            _ => true,
        })
}

fn manifest_check_config(package: &cargo_toml::Package<CargoPackageMetadata>, manifest_dir: &Path, deb: &CargoDeb, listener: &dyn Listener) {
    let readme = package.readme().as_path();
    if package.description().is_none() {
        listener.warning("description field is missing in Cargo.toml".to_owned());
    }
    if package.license().is_none() && package.license_file().is_none() {
        listener.warning("license field is missing in Cargo.toml".to_owned());
    }
    if let Some(readme) = readme {
        if deb.extended_description.is_none() && deb.extended_description_file.is_none() && (readme.ends_with(".md") || readme.ends_with(".markdown")) {
            listener.info(format!("extended-description field missing. Using {}, but markdown may not render well.", readme.display()));
        }
    } else {
        for p in &["README.md", "README.markdown", "README.txt", "README"] {
            if manifest_dir.join(p).exists() {
                listener.warning(format!("{p} file exists, but is not specified in `readme` Cargo.toml field"));
                break;
            }
        }
    }
}

fn manifest_extended_description(desc: Option<String>, desc_file: Option<&Path>) -> CDResult<Option<String>> {
    Ok(if desc.is_some() {
        desc
    } else if let Some(desc_file) = desc_file {
        Some(fs::read_to_string(desc_file)
            .map_err(|err| CargoDebError::IoFile(
                    "unable to read extended description from file", err, PathBuf::from(desc_file)))?)
    } else {
        None
    })
}

fn manifest_license_file(package: &cargo_toml::Package<CargoPackageMetadata>, license_file: Option<&LicenseFile>) -> CDResult<(Option<PathBuf>, usize)> {
    Ok(match license_file {
        Some(LicenseFile::Vec(args)) => {
            let mut args = args.iter();
            let file = args.next();
            let lines = if let Some(lines) = args.next() {
                lines.parse().map_err(|e| CargoDebError::NumParse("invalid number of lines", e))?
            } else {0};
            (file.map(|s|s.into()), lines)
        },
        Some(LicenseFile::String(s)) => {
            (Some(s.into()), 0)
        }
        None => {
            (package.license_file().as_ref().map(|s| s.into()), 0)
        }
    })
}

impl Config {
fn take_assets(&mut self, package: &cargo_toml::Package<CargoPackageMetadata>, assets: Option<Vec<Vec<String>>>, build_targets: &[CargoMetadataTarget], profile: &str, listener: &dyn Listener) -> CDResult<()> {
    let assets = if let Some(assets) = assets {
        let profile_target_dir = format!("target/{profile}");
        // Treat all explicit assets as unresolved until after the build step
        let mut unresolved_assets = Vec::with_capacity(assets.len());
        for mut asset_line in assets {
            let mut asset_parts = asset_line.drain(..);
            let source_path = PathBuf::from(asset_parts.next()
                .ok_or("missing path (first array entry) for asset in Cargo.toml")?);
            if source_path.starts_with("target/debug/") {
                listener.warning(format!("Packaging of development-only binaries is intentionally unsupported in cargo-deb.
Please only use `target/release/` directory for built products, not `{}`.
To add debug information or additional assertions use `[profile.release]` in `Cargo.toml` instead.
This will be hard error in a future release of cargo-deb.", source_path.display()));
            }
            let (is_built, source_path) = if let Ok(rel_path) = source_path.strip_prefix(&profile_target_dir) {
                (self.is_built_file_in_package(&rel_path, build_targets), self.path_in_build(rel_path, profile))
            } else {
                (IsBuilt::No, self.path_in_package(&source_path))
            };
            let target_path = PathBuf::from(asset_parts.next().ok_or("missing target (second array entry) for asset in Cargo.toml. Use something like \"usr/local/bin/\".")?);
            let chmod = u32::from_str_radix(&asset_parts.next().ok_or("missing chmod (third array entry) for asset in Cargo.toml. Use an octal string like \"777\".")?, 8)
                .map_err(|e| CargoDebError::NumParse("unable to parse chmod argument", e))?;

            unresolved_assets.push(UnresolvedAsset {
                source_path,
                c: AssetCommon { target_path, chmod, is_built },
            })
        }
        Assets::with_unresolved_assets(unresolved_assets)
    } else {
        let mut implied_assets: Vec<_> = build_targets.iter()
            .filter_map(|t| {
                if t.crate_types.iter().any(|ty| ty == "bin") && t.kind.iter().any(|k| k == "bin") {
                    Some(Asset::new(
                        AssetSource::Path(self.path_in_build(&t.name, profile)),
                        Path::new("usr/bin").join(&t.name),
                        0o755,
                        self.is_built_file_in_package(t.name.as_ref(), build_targets),
                    ))
                } else if t.crate_types.iter().any(|ty| ty == "cdylib") && t.kind.iter().any(|k| k == "cdylib") {
                    // FIXME: std has constants for the host arch, but not for cross-compilation
                    let lib_name = format!("{DLL_PREFIX}{}{DLL_SUFFIX}", t.name);
                    Some(Asset::new(
                        AssetSource::Path(self.path_in_build(&lib_name, profile)),
                        Path::new("usr/lib").join(lib_name),
                        0o644,
                        self.is_built_file_in_package(t.name.as_ref(), build_targets),
                    ))
                } else {
                    None
                }
            })
            .collect();
        if let OptionalFile::Path(readme) = package.readme() {
            let path = PathBuf::from(readme);
            let target_path = Path::new("usr/share/doc")
                .join(&package.name)
                .join(path.file_name().ok_or("bad README path")?);
            implied_assets.push(Asset::new(AssetSource::Path(path), target_path, 0o644, IsBuilt::No));
        }
        Assets::with_resolved_assets(implied_assets)
    };
    if assets.is_empty() {
        return Err("No binaries or cdylibs found. The package is empty. Please specify some assets to package in Cargo.toml".into());
    }
    self.assets = assets;
    Ok(())
}
    fn is_built_file_in_package(&self, rel_path: &Path, build_targets: &[CargoMetadataTarget]) -> IsBuilt {
        let source_name = rel_path.file_name().expect("asset filename").to_str().expect("utf-8 names");
        let source_name = source_name.strip_suffix(EXE_SUFFIX).unwrap_or(source_name);
        if build_targets.iter().filter(|t| t.name == source_name).any(|t| t.src_path.starts_with(&self.package_manifest_dir)) {
            IsBuilt::SamePackage
        } else {
            IsBuilt::Workspace
        }
    }
}


/// Debian-compatible version of the semver version
fn manifest_version_string(package: &cargo_toml::Package<CargoPackageMetadata>, revision: Option<String>) -> String {
    let debianized_version;
    let mut version = package.version();

    // Make debian's version ordering (newer versions) more compatible with semver's.
    // Keep "semver-1" and "semver-xxx" as-is (assuming these are irrelevant, or debian revision already),
    // but change "semver-beta.1" to "semver~beta.1"
    let mut parts = version.splitn(2, '-');
    let semver_main = parts.next().unwrap();
    if let Some(semver_pre) = parts.next() {
        let pre_ascii = semver_pre.as_bytes();
        if pre_ascii.iter().any(|c| !c.is_ascii_digit()) && pre_ascii.iter().any(|c| c.is_ascii_digit()) {
            debianized_version = format!("{}~{}", semver_main, semver_pre);
            version = &debianized_version;
        }
    }

    if let Some(revision) = revision {
        format!("{}-{}", version, revision)
    } else {
        version.to_owned()
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
struct CargoPackageMetadata {
    pub deb: Option<CargoDeb>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum LicenseFile {
    String(String),
    Vec(Vec<String>),
}

#[derive(Deserialize)]
#[derive(Clone, Debug)]
#[serde(untagged)]
enum SystemUnitsSingleOrMultiple {
    Single(SystemdUnitsConfig),
    Multi(Vec<SystemdUnitsConfig>)
}


#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct CargoDeb {
    pub name: Option<String>,
    pub maintainer: Option<String>,
    pub copyright: Option<String>,
    pub license_file: Option<LicenseFile>,
    pub changelog: Option<String>,
    pub depends: Option<String>,
    pub pre_depends: Option<String>,
    pub recommends: Option<String>,
    pub suggests: Option<String>,
    pub enhances: Option<String>,
    pub conflicts: Option<String>,
    pub breaks: Option<String>,
    pub replaces: Option<String>,
    pub provides: Option<String>,
    pub extended_description: Option<String>,
    pub extended_description_file: Option<String>,
    pub section: Option<String>,
    pub priority: Option<String>,
    pub revision: Option<String>,
    pub conf_files: Option<Vec<String>>,
    pub assets: Option<Vec<Vec<String>>>,
    pub triggers_file: Option<String>,
    pub maintainer_scripts: Option<String>,
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
    pub separate_debug_symbols: Option<bool>,
    pub preserve_symlinks: Option<bool>,
    pub systemd_units: Option<SystemUnitsSingleOrMultiple>,
    pub variants: Option<HashMap<String, CargoDeb>>,
}

impl CargoDeb {
    fn inherit_from(self, parent: CargoDeb) -> CargoDeb {
        CargoDeb {
            name: self.name.or(parent.name),
            maintainer: self.maintainer.or(parent.maintainer),
            copyright: self.copyright.or(parent.copyright),
            license_file: self.license_file.or(parent.license_file),
            changelog: self.changelog.or(parent.changelog),
            depends: self.depends.or(parent.depends),
            pre_depends: self.pre_depends.or(parent.pre_depends),
            recommends: self.recommends.or(parent.recommends),
            suggests: self.suggests.or(parent.suggests),
            enhances: self.enhances.or(parent.enhances),
            conflicts: self.conflicts.or(parent.conflicts),
            breaks: self.breaks.or(parent.breaks),
            replaces: self.replaces.or(parent.replaces),
            provides: self.provides.or(parent.provides),
            extended_description: self.extended_description.or(parent.extended_description),
            extended_description_file: self.extended_description_file.or(parent.extended_description_file),
            section: self.section.or(parent.section),
            priority: self.priority.or(parent.priority),
            revision: self.revision.or(parent.revision),
            conf_files: self.conf_files.or(parent.conf_files),
            assets: self.assets.or(parent.assets),
            triggers_file: self.triggers_file.or(parent.triggers_file),
            maintainer_scripts: self.maintainer_scripts.or(parent.maintainer_scripts),
            features: self.features.or(parent.features),
            default_features: self.default_features.or(parent.default_features),
            separate_debug_symbols: self.separate_debug_symbols.or(parent.separate_debug_symbols),
            preserve_symlinks: self.preserve_symlinks.or(parent.preserve_symlinks),
            systemd_units: self.systemd_units.or(parent.systemd_units),
            variants: self.variants.or(parent.variants),
        }
    }
}

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
    resolve: CargoMetadataResolve,
    #[serde(default)]
    workspace_members: Vec<String>,
    target_directory: String,
    #[serde(default)]
    workspace_root: String,
}

#[derive(Deserialize)]
struct CargoMetadataResolve {
    root: Option<String>,
}

#[derive(Deserialize)]
struct CargoMetadataPackage {
    pub id: String,
    pub name: String,
    pub targets: Vec<CargoMetadataTarget>,
    pub manifest_path: String,
}

#[derive(Deserialize)]
struct CargoMetadataTarget {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: PathBuf,
}

/// Returns the path of the `Cargo.toml` that we want to build.
fn cargo_metadata(manifest_path: &Path) -> CDResult<CargoMetadata> {
    let mut cmd = Command::new("cargo");
    cmd.arg("metadata");
    cmd.arg("--format-version=1");
    cmd.arg("--manifest-path"); cmd.arg(manifest_path);

    let output = cmd.output()
        .map_err(|e| CargoDebError::CommandFailed(e, "cargo (is it in your PATH?)"))?;
    if !output.status.success() {
        return Err(CargoDebError::CommandError("cargo", "metadata".to_owned(), output.stderr));
    }

    let stdout = String::from_utf8(output.stdout).unwrap();
    let metadata = serde_json::from_str(&stdout)?;
    Ok(metadata)
}

/// Format conffiles section, ensuring each path has a leading slash
///
/// Starting with [dpkg 1.20.1](https://github.com/guillemj/dpkg/blob/68ab722604217d3ab836276acfc0ae1260b28f5f/debian/changelog#L393),
/// which is what Ubuntu 21.04 uses, relative conf-files are no longer
/// accepted (the deb-conffiles man page states that "they should be listed as
/// absolute pathnames"). So we prepend a leading slash to the given strings
/// as needed
fn format_conffiles<S: AsRef<str>>(files: &[S]) -> String {
    files.iter().fold(String::new(), |mut acc, x| {
        let pth = x.as_ref();
        if !pth.starts_with('/') {
            acc.push('/');
        }
        acc + pth + "\n"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::tests::add_test_fs_paths;

    #[test]
    fn match_arm_arch() {
        assert_eq!("armhf", debian_architecture_from_rust_triple("arm-unknown-linux-gnueabihf"));
    }

    #[test]
    fn arch_spec() {
        use ArchSpec::*;
        // req
        assert_eq!(
            get_architecture_specification("libjpeg64-turbo [armhf]").expect("arch"),
            ("libjpeg64-turbo".to_owned(), Some(Require("armhf".to_owned()))));
        // neg
        assert_eq!(
            get_architecture_specification("libjpeg64-turbo [!amd64]").expect("arch"),
            ("libjpeg64-turbo".to_owned(), Some(NegRequire("amd64".to_owned()))));
    }

    #[test]
    fn assets() {
        let a = Asset::new(
            AssetSource::Path(PathBuf::from("target/release/bar")),
            PathBuf::from("baz/"),
            0o644,
            IsBuilt::SamePackage,
        );
        assert_eq!("baz/bar", a.c.target_path.to_str().unwrap());
        assert!(a.c.is_built != IsBuilt::No);

        let a = Asset::new(
            AssetSource::Path(PathBuf::from("foo/bar")),
            PathBuf::from("/baz/quz"),
            0o644,
            IsBuilt::No,
        );
        assert_eq!("baz/quz", a.c.target_path.to_str().unwrap());
        assert!(a.c.is_built == IsBuilt::No);
    }

    /// Tests that getting the debug filename from a path returns the same path
    /// with ".debug" appended
    #[test]
    fn test_debug_filename() {
        let path = Path::new("/my/test/file");
        assert_eq!(debug_filename(path), Path::new("/my/test/file.debug"));
    }

    /// Tests that getting the debug target for an Asset that `is_built` returns
    /// the path "/usr/lib/debug/<path-to-target>.debug"
    #[test]
    fn test_debug_target_ok() {
        let a = Asset::new(
            AssetSource::Path(PathBuf::from("target/release/bar")),
            PathBuf::from("/usr/bin/baz/"),
            0o644,
            IsBuilt::SamePackage,
        );
        let debug_target = a.c.debug_target().expect("Got unexpected None");
        assert_eq!(debug_target, Path::new("/usr/lib/debug/usr/bin/baz/bar.debug"));
    }

    /// Tests that getting the debug target for an Asset that `is_built` and that
    /// has a relative path target returns the path "/usr/lib/debug/<path-to-target>.debug"
    #[test]
    fn test_debug_target_ok_relative() {
        let a = Asset::new(
            AssetSource::Path(PathBuf::from("target/release/bar")),
            PathBuf::from("baz/"),
            0o644,
            IsBuilt::Workspace,
        );
        let debug_target = a.c.debug_target().expect("Got unexpected None");
        assert_eq!(debug_target, Path::new("/usr/lib/debug/baz/bar.debug"));
    }

    /// Tests that getting the debug target for an Asset that with `is_built` false
    /// returns None
    #[test]
    fn test_debug_target_not_built() {
        let a = Asset::new(
            AssetSource::Path(PathBuf::from("target/release/bar")),
            PathBuf::from("baz/"),
            0o644,
            IsBuilt::No,
        );

        assert_eq!(a.c.debug_target(), None);
    }

    /// Tests that debug_source() for an AssetSource::Path returns the same path
    /// but with ".debug" appended
    #[test]
    fn test_debug_source_path() {
        let a = AssetSource::Path(PathBuf::from("target/release/bar"));

        let debug_source = a.debug_source().expect("Got unexpected None");
        assert_eq!(debug_source, Path::new("target/release/bar.debug"));
    }

    /// Tests that debug_source() for an AssetSource::Data returns None
    #[test]
    fn test_debug_source_data() {
        let data: Vec<u8> = Vec::new();
        let a = AssetSource::Data(data);

        assert_eq!(a.debug_source(), None);
    }

    fn to_canon_static_str(s: &str) -> &'static str {
        let cwd = std::env::current_dir().unwrap();
        let abs_path = cwd.join(s);
        let abs_path_string = abs_path.to_string_lossy().into_owned();
        Box::leak(abs_path_string.into_boxed_str())
    }

    #[test]
    fn add_systemd_assets_with_no_config_does_nothing() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().return_const(());

        // supply a systemd unit file as if it were available on disk
        let _g = add_test_fs_paths(&[to_canon_static_str("cargo-deb.service")]);

        let config = Config::from_manifest(Path::new("Cargo.toml"), None, None, None, None, None, None, &mock_listener, "release").unwrap();

        let num_unit_assets = config.assets.resolved.iter()
            .filter(|a| a.c.target_path.starts_with("lib/systemd/system/"))
            .count();

        assert_eq!(0, num_unit_assets);
    }

    #[test]
    fn add_systemd_assets_with_config_adds_unit_assets() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().return_const(());

        // supply a systemd unit file as if it were available on disk
        let _g = add_test_fs_paths(&[to_canon_static_str("cargo-deb.service")]);

        let mut config = Config::from_manifest(Path::new("Cargo.toml"), None, None, None, None, None, None, &mock_listener, "release").unwrap();

        config.systemd_units.get_or_insert(vec![SystemdUnitsConfig::default()]);
        config.maintainer_scripts.get_or_insert(PathBuf::new());

        config.add_systemd_assets().unwrap();

        let num_unit_assets = config.assets.resolved
            .iter()
            .filter(|a| a.c.target_path.starts_with("lib/systemd/system/"))
            .count();

        assert_eq!(1, num_unit_assets);
    }

    #[test]
    fn format_conffiles_empty() {
        let actual = format_conffiles::<String>(&[]);
        assert_eq!("", actual);
    }

    #[test]
    fn format_conffiles_one() {
        let actual = format_conffiles(&["/etc/my-pkg/conf.toml"]);
        assert_eq!("/etc/my-pkg/conf.toml\n", actual);
    }

    #[test]
    fn format_conffiles_multiple() {
        let actual = format_conffiles(&["/etc/my-pkg/conf.toml", "etc/my-pkg/conf2.toml"]);

        assert_eq!("/etc/my-pkg/conf.toml\n/etc/my-pkg/conf2.toml\n", actual);
    }
}

#[test]
fn deb_ver() {
    let mut c = cargo_toml::Package::new("test", "1.2.3-1");
    assert_eq!("1.2.3-1", manifest_version_string(&c, None));
    assert_eq!("1.2.3-1-2", manifest_version_string(&c, Some("2".into())));
    c.version = cargo_toml::Inheritable::Set("1.2.0-beta.3".into());
    assert_eq!("1.2.0~beta.3", manifest_version_string(&c, None));
    assert_eq!("1.2.0~beta.3-4", manifest_version_string(&c, Some("4".into())));
    c.version = cargo_toml::Inheritable::Set("1.2.0-new".into());
    assert_eq!("1.2.0-new", manifest_version_string(&c, None));
    assert_eq!("1.2.0-new-11", manifest_version_string(&c, Some("11".into())));
}
