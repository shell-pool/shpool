//! This crate defines `struct`s that can be deserialized with Serde
//! to load and inspect `Cargo.toml` metadata.
//!
//! See `Manifest::from_slice`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryFrom;
use std::fmt::Display;
use std::mem::take;
use std::path::{Path, PathBuf};
use std::{fs, io};
pub use toml::Value;

pub type DepsSet = BTreeMap<String, Dependency>;
pub type TargetDepsSet = BTreeMap<String, Target>;
pub type FeatureSet = BTreeMap<String, Vec<String>>;
pub type PatchSet = BTreeMap<String, DepsSet>;

mod afs;
mod error;
mod inheritable;
pub use crate::afs::*;
pub use crate::error::Error;
pub use crate::inheritable::Inheritable;

/// The top-level `Cargo.toml` structure
///
/// The `Metadata` is a type for `[package.metadata]` table. You can replace it with
/// your own struct type if you use the metadata and don't want to use the catch-all `Value` type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Manifest<Metadata = Value> {
    pub package: Option<Package<Metadata>>,
    pub workspace: Option<Workspace<Metadata>>,
    #[serde(default, skip_serializing_if = "DepsSet::is_empty")]
    pub dependencies: DepsSet,
    #[serde(default, skip_serializing_if = "DepsSet::is_empty")]
    pub dev_dependencies: DepsSet,
    #[serde(default, skip_serializing_if = "DepsSet::is_empty")]
    pub build_dependencies: DepsSet,
    #[serde(default, skip_serializing_if = "TargetDepsSet::is_empty")]
    pub target: TargetDepsSet,
    #[serde(default, skip_serializing_if = "FeatureSet::is_empty")]
    pub features: FeatureSet,

    #[serde(default, skip_serializing_if = "DepsSet::is_empty")]
    #[deprecated(note = "Cargo recommends patch instead")]
    pub replace: DepsSet,

    #[serde(default, skip_serializing_if = "PatchSet::is_empty")]
    pub patch: PatchSet,

    /// Note that due to autolibs feature this is not the complete list
    /// unless you run `complete_from_path`
    pub lib: Option<Product>,
    #[serde(default, skip_serializing_if = "Profiles::should_skip_serializing")]
    pub profile: Profiles,

    #[serde(default, skip_serializing_if = "Badges::should_skip_serializing")]
    pub badges: Badges,

    /// Note that due to autobins feature this is not the complete list
    /// unless you run `complete_from_path`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin: Vec<Product>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bench: Vec<Product>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test: Vec<Product>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub example: Vec<Product>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Workspace<Metadata = Value> {
    #[serde(default)]
    pub members: Vec<String>,

    /// Template for inheritance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageTemplate>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_members: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolver: Option<Resolver>,

    #[serde(default, skip_serializing_if = "DepsSet::is_empty")]
    pub dependencies: DepsSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PackageTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<Edition>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_file: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Publish::is_default")]
    pub publish: Publish,

    #[serde(default, skip_serializing_if = "OptionalFile::is_default")]
    pub readme: OptionalFile,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

fn default_true() -> bool {
    true
}

fn is_default<T: Default + Copy + PartialEq>(val: &T) -> bool {
    *val == T::default()
}

fn is_true(val: &bool) -> bool {
    *val
}

fn is_false(val: &bool) -> bool {
    !*val
}

impl Manifest<Value> {
    /// Parse contents of a `Cargo.toml` file already loaded as a byte slice.
    ///
    /// It does not call `complete_from_path`, so may be missing implicit data.
    #[inline(always)]
    pub fn from_slice(cargo_toml_content: &[u8]) -> Result<Self, Error> {
        Self::from_slice_with_metadata(cargo_toml_content)
    }

    /// Parse contents from a `Cargo.toml` file on disk.
    ///
    /// Calls `complete_from_path`.
    #[inline]
    pub fn from_path(cargo_toml_path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_path_with_metadata(cargo_toml_path)
    }

    /// Parse contents of a `Cargo.toml` file loaded as a string
    ///
    /// Note: this is **not** a file name, but file's content. See `from_path`.
    ///
    /// It does not call `complete_from_path`, so may be missing implicit data.
    #[inline(always)]
    pub fn from_str(cargo_toml_content: &str) -> Result<Self, Error> {
        Self::from_slice_with_metadata(cargo_toml_content.as_bytes())
    }
}

impl<Metadata: for<'a> Deserialize<'a>> Manifest<Metadata> {
    /// Parse `Cargo.toml`, and parse its `[package.metadata]` into a custom Serde-compatible type.package
    ///
    /// It does not call `complete_from_path`, so may be missing implicit data.
    pub fn from_slice_with_metadata(cargo_toml_content: &[u8]) -> Result<Self, Error> {
        let cargo_toml_content = std::str::from_utf8(cargo_toml_content).map_err(|_| Error::Other("utf8"))?;
        let mut manifest: Self = toml::from_str(cargo_toml_content)?;
        if manifest.package.is_none() && manifest.workspace.is_none() {
            // Some old crates lack the `[package]` header

            let val: Value = toml::from_str(cargo_toml_content)?;
            if let Some(project) = val.get("project") {
                manifest.package = Some(project.clone().try_into()?);
            } else {
                manifest.package = Some(val.try_into()?);
            }
        }
        Ok(manifest)
    }

    /// Parse contents from `Cargo.toml` file on disk, with custom Serde-compatible metadata type.
    ///
    /// Calls [`complete_from_path`]
    pub fn from_path_with_metadata<P: AsRef<Path>>(cargo_toml_path: P) -> Result<Self, Error> {
        let cargo_toml_path = cargo_toml_path.as_ref();
        let cargo_toml_content = fs::read(cargo_toml_path)?;
        let mut manifest = Self::from_slice_with_metadata(&cargo_toml_content)?;
        manifest.complete_from_path(cargo_toml_path)?;
        Ok(manifest)
    }

    /// `Cargo.toml` doesn't contain explicit information about `[lib]` and `[[bin]]`,
    /// which are inferred based on files on disk.
    ///
    /// This scans the disk to make the data in the manifest as complete as possible.
    ///
    /// It supports workspace inheritance and will search for a root workspace.
    /// Use `complete_from_path_and_workspace` to provide the workspace explicitly.
    pub fn complete_from_path(&mut self, path: &Path) -> Result<(), Error> {
        let manifest_dir = path.parent().ok_or(Error::Other("bad path"))?;
        self.complete_from_abstract_filesystem::<Value, _>(Filesystem::new(manifest_dir), None)
    }

    /// [`complete_from_path`], but allows passing workspace manifest explicitly.
    ///
    /// `workspace_manifest_and_path` is the root workspace manifest already parsed,
    /// and the path is the path to the root workspace's directory.
    /// If it's `None`, the root workspace will be discovered automatically.
    pub fn complete_from_path_and_workspace<WorkspaceMetadataIgnored>(&mut self, package_manifest_path: &Path, workspace_manifest_and_path: Option<(&Manifest<WorkspaceMetadataIgnored>, &Path)>) -> Result<(), Error> {
        let manifest_dir = package_manifest_path.parent().ok_or(Error::Other("bad path"))?;
        self.complete_from_abstract_filesystem(Filesystem::new(manifest_dir), workspace_manifest_and_path)
    }

    /// `Cargo.toml` doesn't contain explicit information about `[lib]` and `[[bin]]`,
    /// which are inferred based on files on disk.
    ///
    /// You can provide any implementation of directory scan, which doesn't have to
    /// be reading straight from disk (might scan a tarball or a git repo, for example).
    ///
    /// If `workspace_manifest_and_path` is set, it will inherit from this workspace.
    /// If it's `None`, it will try to find a workspace if needed.
    pub fn complete_from_abstract_filesystem<WorkspaceMetadataIgnored, Fs: AbstractFilesystem>(
        &mut self, fs: Fs, workspace_manifest_and_path: Option<(&Manifest<WorkspaceMetadataIgnored>, &Path)>
    ) -> Result<(), Error> {
        if let Some((ws, ws_path)) = workspace_manifest_and_path {
            self._inherit_workspace(ws.workspace.as_ref(), ws_path)?;
        } else if let Some(ws) = self.workspace.take() {
            // Manifest may be both a workspace and a package
            self._inherit_workspace(Some(&ws), Path::new(""))?;
            self.workspace = Some(ws);
        } else if self.needs_workspace_inheritance() {
            let (toml, base_path) = fs.read_root_workspace(self.package.as_ref().and_then(|p| p.workspace.as_deref()))
                .map_err(|e| Error::Workspace(Box::new(e.into())))?;
            let manifest = Manifest::from_slice(&toml)
                .map_err(|e| Error::Workspace(Box::new(e)))?;
            self._inherit_workspace(manifest.workspace.as_ref(), &base_path)?;
        }
        self.complete_from_abstract_filesystem_inner(&fs)
    }

    fn needs_workspace_inheritance(&self) -> bool {
        self.package.as_ref().map_or(false, |p| p.needs_workspace_inheritance()) ||
        self.dependencies.values()
            .chain(self.build_dependencies.values())
            .chain(self.dev_dependencies.values())
            .any(|dep| {
                matches!(dep, Dependency::Inherited(_))
            })
    }

    /// Copy workspace-inheritable properties from the `workspace_manifest`.
    ///
    /// `workspace_base_path` should be an absolute path to a directory where the workspace manifest is located.
    /// Used as a base for `readme` and `license-file`.
    #[deprecated(note = "this functionality has been merged into `complete_from_path_and_workspace` or `complete_from_abstract_filesystem`")]
    pub fn inherit_workspace<Ignored>(&mut self, workspace_manifest: &Manifest<Ignored>, workspace_base_path: &Path) -> Result<(), Error> {
        self._inherit_workspace(workspace_manifest.workspace.as_ref(), workspace_base_path)
    }

    fn _inherit_workspace<Ignored>(&mut self, workspace: Option<&Workspace<Ignored>>, workspace_base_path: &Path) -> Result<(), Error> {
        let workspace_base_path = if workspace_base_path.file_name() == Some("Cargo.toml".as_ref()) {
            workspace_base_path.parent().ok_or(Error::Other("bad path"))?
        } else {
            workspace_base_path
        };

        inherit_dependencies(&mut self.dependencies, workspace, workspace_base_path)?;
        inherit_dependencies(&mut self.build_dependencies, workspace, workspace_base_path)?;
        inherit_dependencies(&mut self.dev_dependencies, workspace, workspace_base_path)?;

        let package = match &mut self.package {
            Some(p) => p,
            None => return Ok(()),
        };
        if let Some(ws) = workspace.and_then(|w| w.package.as_ref()) {
            Self::inherit_package_properties(package, ws, workspace_base_path)?;
        }

        if package.needs_workspace_inheritance() {
            return Err(Error::WorkspaceIntegrity(format!("not all fields of `{}` have been present in workspace.package", package.name())));
        }
        Ok(())
    }

    fn inherit_package_properties(package: &mut Package<Metadata>, ws: &PackageTemplate, workspace_base_path: &Path) -> Result<(), Error> {
        fn maybe_inherit<T: Clone>(to: Option<&mut Inheritable<T>>, from: Option<&T>) {
            if let Some(from) = from {
                if let Some(to) = to {
                    to.inherit(from);
                }
            }
        }
        fn inherit<T: Clone>(to: &mut Inheritable<T>, from: Option<&T>) {
            if let Some(from) = from {
                to.inherit(from);
            }
        }
        inherit(&mut package.authors, ws.authors.as_ref());
        inherit(&mut package.categories, ws.categories.as_ref());
        inherit(&mut package.edition, ws.edition.as_ref());
        inherit(&mut package.exclude, ws.exclude.as_ref());
        inherit(&mut package.include, ws.include.as_ref());
        inherit(&mut package.keywords, ws.keywords.as_ref());
        inherit(&mut package.version, ws.version.as_ref());
        maybe_inherit(package.description.as_mut(), ws.description.as_ref());
        maybe_inherit(package.documentation.as_mut(), ws.documentation.as_ref());
        maybe_inherit(package.homepage.as_mut(), ws.homepage.as_ref());
        maybe_inherit(package.license.as_mut(), ws.license.as_ref());
        maybe_inherit(package.repository.as_mut(), ws.repository.as_ref());
        maybe_inherit(package.rust_version.as_mut(), ws.rust_version.as_ref());
        package.publish.inherit(&ws.publish);
        match (&mut package.readme, &ws.readme) {
            (r @ Inheritable::Inherited { .. }, flag @ OptionalFile::Flag(_)) => {
                r.set(flag.clone())
            },
            (r @ Inheritable::Inherited { .. }, OptionalFile::Path(path)) => {
                r.set(OptionalFile::Path(workspace_base_path.join(path)))
            },
            _ => {},
        }
        Ok(match (package.license_file.as_mut(), ws.license_file.as_ref()) {
            (Some(f), Some(ws)) => {
                f.set(workspace_base_path.join(ws))
            },
            _ => {},
        })
    }


    fn complete_from_abstract_filesystem_inner(&mut self, fs: &dyn AbstractFilesystem) -> Result<(), Error> {
        let package = match &self.package {
            Some(p) => p,
            None => return Ok(()),
        };

        let src = match fs.file_names_in("src") {
            Ok(src) => src,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Default::default(),
            Err(err) => return Err(err.into()),
        };

        if let Some(ref mut lib) = self.lib {
            lib.required_features.clear(); // not applicable
        }

        let has_path = self.lib.as_ref().map_or(false, |l| l.path.is_some());
        if !has_path && src.contains("lib.rs") {
            let old_lib = self.lib.take().unwrap_or_default();
            self.lib = Some(Product {
                name: if let Some(name) = old_lib.name { Some(name)} else {Some(package.name.replace('-', "_"))},
                path: Some("src/lib.rs".to_string()),
                edition: *package.edition.get()?,
                crate_type: vec!["rlib".to_string()],
                ..old_lib
            })
        }

        if package.autobins {
            let mut bin = take(&mut self.bin);
            let (fully_overrided, mut partial_overrided) = self.autoset(&mut bin, "src/bin", fs)?;
            self.bin = bin;

            if src.contains("main.rs") && !fully_overrided.contains("src/main.rs") {
                let rel_path = "src/main.rs".to_string();
                let name = &package.name;

                let product = if let Some(mut product) = partial_overrided.remove(name) {
                    product.path = Some(rel_path);
                    product
                } else {
                    Product {
                        name: Some(name.clone()),
                        path: Some(rel_path),
                        edition: *package.edition.get()?,
                        ..Product::default()
                    }
                };
                self.bin.push(product);
            }
        }

        Self::sort_products(&mut self.bin);

        if package.autoexamples {
            let mut example = take(&mut self.example);
            self.autoset(&mut example, "examples", fs)?;
            self.example = example;
        }

        Self::sort_products(&mut self.example);

        if package.autotests {
            let mut test = take(&mut self.test);
            self.autoset(&mut test, "tests", fs)?;
            self.test = test;
        }

        Self::sort_products(&mut self.test);

        if package.autobenches {
            let mut bench = take(&mut self.bench);
            self.autoset(&mut bench, "benches", fs)?;
            self.bench = bench;
        }

        Self::sort_products(&mut self.bench);

        let package = self.package.as_mut().unwrap();

        if matches!(package.build, None | Some(OptionalFile::Flag(true))) && fs.file_names_in(".").map_or(false, |dir| dir.contains("build.rs")) {
            package.build = Some(OptionalFile::Path("build.rs".into()));
        }

        if matches!(package.readme.get()?, OptionalFile::Flag(true)) {
            let files = fs.file_names_in(".").ok();
            if let Some(name) = files.as_ref().and_then(|dir| {
                dir.get("README.md")
                    .or_else(|| dir.get("README.txt"))
                    .or_else(|| dir.get("README"))
            }) {
                package.readme = Inheritable::Set(OptionalFile::Path(PathBuf::from(&**name)));
            }
        }
        Ok(())
    }

    /// Return the set of path overrided in `Cargo.toml`.
    fn autoset(
        &self,
        out: &mut Vec<Product>,
        dir: &str,
        fs: &dyn AbstractFilesystem,
    ) -> Result<(BTreeSet<String>, BTreeMap<String, Product>), Error> {
        let fully_overrided: BTreeSet<_> = out.iter()
            .filter_map(|product| product.path.clone())
            .collect();

        let mut partial_overrided: BTreeMap<String, Product> = out.iter()
            .filter_map(|product| {
                match (&product.path, &product.name)  {
                    (None, Some(name)) => {
                        Some((name.clone(), product.clone()))
                    },
                    _ => None
                }
            })
            .collect();

        // Remove partially overrided items
        out.retain(|product| product.path.is_some());

        if let Some(ref package) = self.package {
            if let Ok(bins) = fs.file_names_in(dir) {
                for name in bins {
                    let rel_path = format!("{dir}/{name}");

                    if name.ends_with(".rs") {
                        if !fully_overrided.contains(&rel_path) {
                            let name = name.trim_end_matches(".rs");

                            let product = if let Some(mut product) = partial_overrided.remove(name) {
                                product.path = Some(rel_path);
                                product
                            } else {
                                Product {
                                    name: Some(name.to_string()),
                                    path: Some(rel_path),
                                    edition: *package.edition.get()?,
                                    ..Product::default()
                                }
                            };
                            out.push(product);
                        }
                    } else if let Ok(sub) = fs.file_names_in(&rel_path) {
                        let rel_path = format!("{rel_path}/main.rs");

                        if sub.contains("main.rs") && !fully_overrided.contains(&rel_path) {
                            let product = if let Some(mut product) = partial_overrided.remove(&*name) {
                                product.path = Some(rel_path);
                                product
                            } else {
                                Product {
                                    name: Some(name.into()),
                                    path: Some(rel_path),
                                    edition: *package.edition.get()?,
                                    ..Product::default()
                                }
                            };
                            out.push(product);
                        }
                    }
                }
            }
        }
        Ok((fully_overrided, partial_overrided))
    }

    /// ensure bins are deterministic
    fn sort_products(products: &mut [Product]) {
        products.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    }

    /// Panics if it's not a package (only a workspace).
    ///
    /// You can access `.package` field directly to handle the `Option`.
    #[track_caller]
    #[inline]
    pub fn package(&self) -> &Package<Metadata> {
        self.package.as_ref().expect("not a package")
    }
}

fn inherit_dependencies<Ignored>(deps_to_inherit: &mut BTreeMap<String, Dependency>, workspace: Option<&Workspace<Ignored>>, workspace_base_path: &Path) -> Result<(), Error> {
    for (key, dep) in deps_to_inherit {
        if let Dependency::Inherited(overrides) = dep {
            let template = workspace.and_then(|ws| ws.dependencies.get(key))
                .ok_or_else(|| Error::WorkspaceIntegrity(format!("workspace dependencies are missing `{key}`")))?;
            let mut overrides = overrides.clone();
            *dep = template.clone();
            if overrides.optional {
                dep.detail_mut().optional = true;
            }
            if !overrides.features.is_empty() {
                dep.detail_mut().features.append(&mut overrides.features);
            }
            if let Dependency::Detailed(dep) = dep {
                dep.inherited = true;
                if let Some(path) = &mut dep.path {
                    *path = workspace_base_path.join(&path).display().to_string();
                }
            }
        }
    }
    Ok(())
}

impl<Metadata: Default> Default for Manifest<Metadata> {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            package: Default::default(),
            workspace: Default::default(),
            dependencies: Default::default(),
            dev_dependencies: Default::default(),
            build_dependencies: Default::default(),
            target: Default::default(),
            features: Default::default(),
            replace: Default::default(),
            patch: Default::default(),
            lib: Default::default(),
            profile: Default::default(),
            badges: Default::default(),
            bin: Default::default(),
            bench: Default::default(),
            test: Default::default(),
            example: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Profiles {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bench: Option<Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Profile>,

    #[serde(flatten)]
    pub custom: HashMap<String, Profile>,
}

impl Profiles {
    /// Determine whether or not a Profiles struct should be serialized
    fn should_skip_serializing(&self) -> bool {
        self.release.is_none()
            && self.dev.is_none()
            && self.test.is_none()
            && self.bench.is_none()
            && self.doc.is_none()
            && self.custom.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "toml::Value")]
pub enum DebugSetting {
    /// 0 or false
    None = 0,
    /// 1 = line tables only
    Lines = 1,
    /// 2 or true
    Full = 2,
}

impl TryFrom<Value> for DebugSetting {
    type Error = Error;
    fn try_from(v: Value) -> Result<Self, Error> {
        Ok(match v {
            Value::Boolean(b) => if b { Self::Full } else { Self::None },
            Value::Integer(n) => match n {
                0 => Self::None,
                1 => Self::Lines,
                2 => Self::Full,
                _ => return Err(Error::Other("wrong number for debug setting")),
            },
            _ => return Err(Error::Other("wrong data type for debug setting")),
        })
    }
}

impl Serialize for DebugSetting {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::None => serializer.serialize_bool(false),
            Self::Lines => serializer.serialize_i8(1),
            Self::Full => serializer.serialize_bool(true),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "toml::Value")]
pub enum StripSetting {
    /// false
    None,
    Debuginfo,
    /// true
    Symbols,
}

impl Serialize for StripSetting {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::None => serializer.serialize_bool(false),
            Self::Debuginfo => serializer.serialize_str("debuginfo"),
            Self::Symbols => serializer.serialize_bool(true),
        }
    }
}

impl TryFrom<Value> for StripSetting {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self, Error> {
        Ok(match v {
            Value::Boolean(b) => if b { Self::Symbols } else { Self::None },
            Value::String(s) => match s.as_str() {
                "none" => Self::None,
                "debuginfo" => Self::Debuginfo,
                "symbols" => Self::Symbols,
                _ => return Err(Error::Other("strip setting has unknown string value")),
            },
            _ => return Err(Error::Other("wrong data type for strip setting")),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "toml::Value")]
pub enum LtoSetting {
    /// off
    None,
    /// false
    ThinLocal,
    Thin,
    /// True
    Fat,
}

impl Serialize for LtoSetting {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::None => serializer.serialize_str("off"),
            Self::ThinLocal => serializer.serialize_bool(false),
            Self::Thin => serializer.serialize_str("thin"),
            Self::Fat => serializer.serialize_bool(true),
        }
    }
}

impl TryFrom<Value> for LtoSetting {
    type Error = Error;

    fn try_from(v: Value) -> Result<Self, Error> {
        Ok(match v {
            Value::Boolean(b) => if b { Self::Fat } else { Self::ThinLocal },
            Value::String(s) => match s.as_str() {
                "off" | "n" | "no" => Self::None,
                "thin" => Self::Thin,
                "fat" | "on" | "y" | "yes" => Self::Fat,
                _ => return Err(Error::Other("lto setting has unknown string value")),
            },
            _ => return Err(Error::Other("wrong data type for lto setting")),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
    /// num or z, s
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opt_level: Option<Value>,

    /// 0,1,2 or bool
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugSetting>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_debuginfo: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpath: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lto: Option<LtoSetting>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_assertions: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codegen_units: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_checks: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<StripSetting>,

    /// profile overrides
    #[serde(default)]
    pub package: BTreeMap<String, Value>,

    /// profile overrides
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_override: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Cargo uses the term "target" for both "target platform" and "build target" (the thing to build),
/// which makes it ambigous.
/// Here Cargo's bin/lib **target** is renamed to **product**.
pub struct Product {
    /// This field points at where the crate is located, relative to the `Cargo.toml`.
    pub path: Option<String>,

    /// The name of a product is the name of the library or binary that will be generated.
    /// This is defaulted to the name of the package, with any dashes replaced
    /// with underscores. (Rust `extern crate` declarations reference this name;
    /// therefore the value must be a valid Rust identifier to be usable.)
    pub name: Option<String>,

    /// A flag for enabling unit tests for this product. This is used by `cargo test`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub test: bool,

    /// A flag for enabling documentation tests for this product. This is only relevant
    /// for libraries, it has no effect on other sections. This is used by
    /// `cargo test`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub doctest: bool,

    /// A flag for enabling benchmarks for this product. This is used by `cargo bench`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub bench: bool,

    /// A flag for enabling documentation of this product. This is used by `cargo doc`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub doc: bool,

    /// If the product is meant to be a compiler plugin, this field must be set to true
    /// for Cargo to correctly compile it and make it available for all dependencies.
    #[serde(default, skip_serializing_if = "is_false")]
    pub plugin: bool,

    /// If the product is meant to be a "macros 1.1" procedural macro, this field must
    /// be set to true.
    #[serde(default, alias = "proc_macro", alias = "proc-macro", skip_serializing_if = "is_false")]
    pub proc_macro: bool,

    /// If set to false, `cargo test` will omit the `--test` flag to rustc, which
    /// stops it from generating a test harness. This is useful when the binary being
    /// built manages the test runner itself.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub harness: bool,

    /// If set then a product can be configured to use a different edition than the
    /// `[package]` is configured to use, perhaps only compiling a library with the
    /// 2018 edition or only compiling one unit test with the 2015 edition. By default
    /// all products are compiled with the edition specified in `[package]`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub edition: Edition,

    /// The available options are "dylib", "rlib", "staticlib", "cdylib", and "proc-macro".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crate_type: Vec<String>,

    /// The required-features field specifies which features the product needs in order to be built.
    /// If any of the required features are not selected, the product will be skipped.
    /// This is only relevant for the `[[bin]]`, `[[bench]]`, `[[test]]`, and `[[example]]` sections,
    /// it has no effect on `[lib]`.
    #[serde(default)]
    pub required_features: Vec<String>,
}

impl Default for Product {
    fn default() -> Self {
        Self {
            path: None,
            name: None,
            test: true,
            doctest: true,
            bench: true,
            doc: true,
            harness: true,
            plugin: false,
            proc_macro: false,
            required_features: Vec::new(),
            crate_type: Vec::new(),
            edition: Edition::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Target {
    #[serde(default)]
    pub dependencies: DepsSet,
    #[serde(default)]
    pub dev_dependencies: DepsSet,
    #[serde(default)]
    pub build_dependencies: DepsSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    /// Version
    Simple(String),
    /// Incomplete data
    Inherited(InheritedDependencyDetail), // order is important for serde
    Detailed(DependencyDetail),
}

impl Dependency {
    #[inline]
    #[must_use] pub fn detail(&self) -> Option<&DependencyDetail> {
        match *self {
            Dependency::Detailed(ref d) => Some(d),
            Dependency::Simple(_) | Dependency::Inherited(_) => None,
        }
    }

    /// Panics if inherited value is not available
    #[inline]
    pub fn detail_mut(&mut self) -> &mut DependencyDetail {
        match self {
            Dependency::Detailed(d) => d,
            Dependency::Simple(ver) => {
                *self = Dependency::Detailed(DependencyDetail {
                    version: Some(ver.clone()),
                    ..Default::default()
                });
                match self {
                    Dependency::Detailed(d) => d,
                    _ => unreachable!(),
                }
            },
            Dependency::Inherited(_) => panic!("dependency not available due to workspace inheritance"),
        }
    }

    #[inline]
    #[track_caller]
    #[must_use] pub fn req(&self) -> &str {
        match *self {
            Dependency::Simple(ref v) => v,
            Dependency::Detailed(ref d) => d.version.as_deref().unwrap_or("*"),
            Dependency::Inherited(_) => panic!("version requirement not available with workspace inheritance"),
        }
    }

    #[inline]
    #[must_use] pub fn req_features(&self) -> &[String] {
        match *self {
            Dependency::Simple(_) => &[],
            Dependency::Detailed(ref d) => &d.features,
            Dependency::Inherited(ref d) => &d.features,
        }
    }

    #[inline]
    #[must_use] pub fn optional(&self) -> bool {
        self.detail().map_or(false, |d| d.optional)
    }

    // `Some` if it overrides the package name.
    // If `None`, use the dependency name as the package name.
    #[inline]
    #[must_use] pub fn package(&self) -> Option<&str> {
        match *self {
            Dependency::Detailed(ref d) => d.package.as_deref(),
            Dependency::Simple(_) | Dependency::Inherited(_) => None,
        }
    }

    // Git URL of this dependency, if any
    #[inline]
    #[must_use] pub fn git(&self) -> Option<&str> {
        self.detail()?.git.as_deref()
    }

    // Git URL of this dependency, if any
    #[inline]
    #[must_use] pub fn git_rev(&self) -> Option<&str> {
        self.detail()?.rev.as_deref()
    }

    // `true` if it's an usual crates.io dependency,
    // `false` if git/path/alternative registry
    #[track_caller]
    #[must_use] pub fn is_crates_io(&self) -> bool {
        match *self {
            Dependency::Simple(_) => true,
            Dependency::Detailed(ref d) => {
                // TODO: allow registry to be set to crates.io explicitly?
                d.path.is_none()
                    && d.registry.is_none()
                    && d.registry_index.is_none()
                    && d.git.is_none()
                    && d.tag.is_none()
                    && d.branch.is_none()
                    && d.rev.is_none()
            },
            Dependency::Inherited(_) => panic!("data not available with workspace inheritance"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DependencyDetail {
    /// Requirement
    pub version: Option<String>,

    pub registry: Option<String>,
    pub registry_index: Option<String>,

    /// This path is usually relative to the crate's manifest, but when using workspace inheritance, it may be relative to the workspace!
    /// When calling `complete_from_path_and_workspace` use absolute path for the workspace manifest, and then this will be corrected to be an absolute
    /// path when inherited from the workspace.
    pub path: Option<String>,

    /// If true, the dependency has been defined at the workspace level, so the `path` is joined with workspace's base path.
    ///
    /// Note that `Dependency::Simple` won't have this flag, even if it was inherited.
    #[serde(skip)]
    pub inherited: bool,

    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,

    #[serde(default)]
    pub features: Vec<String>,

    /// NB: Not allowed at workspace level
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub default_features: bool,

    /// Use this crate name instead of table key
    pub package: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InheritedDependencyDetail {
    #[serde(default)]
    pub features: Vec<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,

    #[serde(skip_serializing_if = "is_false")]
    pub workspace: bool,
}

/// You can replace `Metadata` type with your own
/// to parse into something more useful than a generic toml `Value`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Package<Metadata = Value> {
    /// Careful: some names are uppercase
    pub name: String,

    #[serde(default)]
    pub edition: Inheritable<Edition>,

    /// MSRV 1.x (beware: does not require semver formatting)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<Inheritable<String>>,

    /// e.g. "1.9.0"
    pub version: Inheritable<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<OptionalFile>,

    /// Workspace this package is a member of (`None` if it's implicit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,

    #[serde(default)]
    /// e.g. ["Author <e@mail>", "etc"] Deprecated.
    #[serde(skip_serializing_if = "Inheritable::is_empty")]
    pub authors: Inheritable<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,

    /// A short blurb about the package. This is not rendered in any format when
    /// uploaded to crates.io (aka this is not markdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Inheritable<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Inheritable<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Inheritable<String>>,

    /// This points to a file under the package root (relative to this `Cargo.toml`).
    /// implied if README.md, README.txt or README exists.
    #[serde(default, skip_serializing_if = "Inheritable::is_default")]
    pub readme: Inheritable<OptionalFile>,

    #[serde(default, skip_serializing_if = "Inheritable::is_empty")]
    pub keywords: Inheritable<Vec<String>>,

    /// This is a list of up to five categories where this crate would fit.
    /// e.g. ["command-line-utilities", "development-tools::cargo-plugins"]
    #[serde(default, skip_serializing_if = "Inheritable::is_empty")]
    pub categories: Inheritable<Vec<String>>,

    #[serde(default, skip_serializing_if = "Inheritable::is_empty")]
    pub exclude: Inheritable<Vec<String>>,

    #[serde(default, skip_serializing_if = "Inheritable::is_empty")]
    pub include: Inheritable<Vec<String>>,

    /// e.g. "MIT"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<Inheritable<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_file: Option<Inheritable<PathBuf>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Inheritable<String>>,

    /// The default binary to run by cargo run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_run: Option<String>,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub autobins: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub autoexamples: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub autotests: bool,

    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub autobenches: bool,

    #[serde(default, skip_serializing_if = "Inheritable::is_default")]
    pub publish: Inheritable<Publish>,

    /// "2" is the only useful value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<Resolver>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

#[allow(deprecated)]
impl<Metadata> Package<Metadata> {
    /// Prefer creating it by parsing `Manifest` instead
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Inheritable::Set(version.into()),
            edition: Inheritable::Set(Edition::E2021),
            rust_version: None,
            build: None,
            workspace: None,
            authors: Default::default(),
            links: None,
            description: None,
            homepage: None,
            documentation: None,
            readme: Inheritable::Set(OptionalFile::Flag(true)),
            keywords: Default::default(),
            categories: Default::default(),
            exclude: Default::default(),
            include: Default::default(),
            license: None,
            license_file: None,
            repository: None,
            default_run: None,
            autobins: true,
            autoexamples: true,
            autotests: true,
            autobenches: true,
            publish: Inheritable::Set(Publish::Flag(true)),
            resolver: None,
            metadata: None,
        }
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn authors(&self) -> &[String] {
        self.authors.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn categories(&self) -> &[String] {
        self.categories.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn categories_mut(&mut self) -> &mut Vec<String> {
        self.categories.as_mut().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn description(&self) -> Option<&str> {
        Some(self.description.as_ref()?.as_ref().unwrap())
    }

    #[inline]
    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description.map(Inheritable::Set);
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn documentation(&self) -> Option<&str> {
        Some(self.documentation.as_ref()?.as_ref().unwrap())
    }

    #[inline]
    pub fn set_documentation(&mut self, documentation: Option<String>) {
        self.documentation = documentation.map(Inheritable::Set);
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn edition(&self) -> Edition {
        self.edition.unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn exclude(&self) -> &[String] {
        self.exclude.as_ref().unwrap()
    }
    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn include(&self) -> &[String] {
        self.include.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn homepage(&self) -> Option<&str> {
        Some(self.homepage.as_ref()?.as_ref().unwrap())
    }

    #[inline]
    pub fn set_homepage(&mut self, homepage: Option<String>) {
        self.homepage = homepage.map(Inheritable::Set);
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn keywords(&self) -> &[String] {
        self.keywords.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn license(&self) -> Option<&str> {
        Some(self.license.as_ref()?.as_ref().unwrap())
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn license_file(&self) -> Option<&Path> {
        Some(self.license_file.as_ref()?.as_ref().unwrap())
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn publish(&self) -> &Publish {
        self.publish.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn readme(&self) -> &OptionalFile {
        self.readme.as_ref().unwrap()
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn repository(&self) -> Option<&str> {
        Some(self.repository.as_ref()?.as_ref().unwrap())
    }

    #[inline]
    pub fn set_repository(&mut self, repository: Option<String>) {
        self.repository = repository.map(Inheritable::Set);
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn rust_version(&self) -> Option<&str> {
        Some(self.rust_version.as_ref()?.as_ref().unwrap())
    }

    /// Panics if the field is not available (inherited from a workspace that hasn't been loaded)
    #[track_caller]
    #[inline]
    pub fn version(&self) -> &str {
        self.version.as_ref().unwrap()
    }

    #[inline]
    pub fn links(&self) -> Option<&str> {
        self.links.as_deref()
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    fn needs_workspace_inheritance(&self) -> bool {
        !(self.authors.is_set() &&
        self.categories.is_set() &&
        self.edition.is_set() &&
        self.exclude.is_set() &&
        self.include.is_set() &&
        self.keywords.is_set() &&
        self.version.is_set() &&
        self.description.as_ref().map_or(true, |v| v.is_set()) &&
        self.documentation.as_ref().map_or(true, |v| v.is_set()) &&
        self.homepage.as_ref().map_or(true, |v| v.is_set()) &&
        self.license.as_ref().map_or(true, |v| v.is_set()) &&
        self.license_file.as_ref().map_or(true, |v| v.is_set()) &&
        self.repository.as_ref().map_or(true, |v| v.is_set()) &&
        self.rust_version.as_ref().map_or(true, |v| v.is_set()) &&
        self.publish.is_set() &&
        self.readme.is_set())
    }
}

impl<Metadata: Default> Default for Package<Metadata> {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// Readme of build.rs path
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OptionalFile {
    /// Opt-in to default, or explicit opt-out
    Flag(bool),
    /// Explicit path
    Path(PathBuf),
}

impl Default for OptionalFile {
    #[inline]
    fn default() -> Self {
        Self::Flag(true)
    }
}

impl OptionalFile {
    #[inline]
    fn is_default(&self) -> bool {
        matches!(self, Self::Flag(flag) if *flag)
    }

    #[inline]
    #[must_use] pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(p) => Some(p),
            _ => None,
        }
    }

    #[inline]
    #[must_use] pub fn is_some(&self) -> bool {
        matches!(self, Self::Flag(true) | Self::Path(_))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Publish {
    Flag(bool),
    Registry(Vec<String>),
}

impl Publish {
    fn is_default(&self) -> bool {
        matches!(self, Publish::Flag(flag) if *flag)
    }
}

impl Default for Publish {
    #[inline]
    fn default() -> Self {
        Publish::Flag(true)
    }
}

impl PartialEq<Publish> for bool {
    #[inline]
    fn eq(&self, p: &Publish) -> bool {
        match *p {
            Publish::Flag(flag) => flag == *self,
            Publish::Registry(ref reg) => reg.is_empty() != *self,
        }
    }
}

impl PartialEq<bool> for Publish {
    #[inline]
    fn eq(&self, b: &bool) -> bool {
        b.eq(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Badge {
    pub repository: String,
    #[serde(default = "default_master")]
    pub branch: String,
    pub service: Option<String>,
    pub id: Option<String>,
    pub project_name: Option<String>,
}

fn default_master() -> String {
    "master".to_string()
}

fn ok_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: Deserializer<'de>,
{
    Ok(Deserialize::deserialize(deserializer).unwrap_or_default())
}

/// Deprecated
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Badges {
    /// Appveyor: `repository` is required. `branch` is optional; default is `master`
    /// `service` is optional; valid values are `github` (default), `bitbucket`, and
    /// `gitlab`; `id` is optional; you can specify the appveyor project id if you
    /// want to use that instead. `project_name` is optional; use when the repository
    /// name differs from the appveyor project name.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub appveyor: Option<Badge>,

    /// Circle CI: `repository` is required. `branch` is optional; default is `master`
    #[serde(default, deserialize_with = "ok_or_default")]
    pub circle_ci: Option<Badge>,

    /// GitLab: `repository` is required. `branch` is optional; default is `master`
    #[serde(default, deserialize_with = "ok_or_default")]
    pub gitlab: Option<Badge>,

    /// Travis CI: `repository` in format `"<user>/<project>"` is required.
    /// `branch` is optional; default is `master`
    #[serde(default, deserialize_with = "ok_or_default")]
    #[deprecated(note = "badges are deprecated, and travis is dead")]
    pub travis_ci: Option<Badge>,

    /// Codecov: `repository` is required. `branch` is optional; default is `master`
    /// `service` is optional; valid values are `github` (default), `bitbucket`, and
    /// `gitlab`.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub codecov: Option<Badge>,

    /// Coveralls: `repository` is required. `branch` is optional; default is `master`
    /// `service` is optional; valid values are `github` (default) and `bitbucket`.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub coveralls: Option<Badge>,

    /// Is it maintained resolution time: `repository` is required.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub is_it_maintained_issue_resolution: Option<Badge>,

    /// Is it maintained percentage of open issues: `repository` is required.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub is_it_maintained_open_issues: Option<Badge>,

    /// Maintenance: `status` is required. Available options are `actively-developed`,
    /// `passively-maintained`, `as-is`, `experimental`, `looking-for-maintainer`,
    /// `deprecated`, and the default `none`, which displays no badge on crates.io.
    #[serde(default, deserialize_with = "ok_or_default")]
    pub maintenance: Maintenance,
}

impl Badges {
    #[allow(deprecated)]
    /// Determine whether or not a Profiles struct should be serialized
    fn should_skip_serializing(&self) -> bool {
        self.appveyor.is_none()
            && self.circle_ci.is_none()
            && self.gitlab.is_none()
            && self.travis_ci.is_none()
            && self.codecov.is_none()
            && self.coveralls.is_none()
            && self.is_it_maintained_issue_resolution.is_none()
            && self.is_it_maintained_open_issues.is_none()
            && matches!(self.maintenance.status, MaintenanceStatus::None)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Default, Serialize, Deserialize)]
pub struct Maintenance {
    pub status: MaintenanceStatus,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaintenanceStatus {
    None,
    ActivelyDeveloped,
    PassivelyMaintained,
    AsIs,
    Experimental,
    LookingForMaintainer,
    Deprecated,
}

impl Default for MaintenanceStatus {
    fn default() -> Self {
        MaintenanceStatus::None
    }
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum Edition {
    #[serde(rename = "2015")]
    E2015 = 2015,
    #[serde(rename = "2018")]
    E2018 = 2018,
    #[serde(rename = "2021")]
    E2021 = 2021,
}

impl Default for Edition {
    fn default() -> Self {
        Self::E2015
    }
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum Resolver {
    #[serde(rename = "1")]
    V1 = 1,
    #[serde(rename = "2")]
    V2 = 2,
}

impl Display for Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Resolver::V1 => "1",
            Resolver::V2 => "2",
        })
    }
}

impl Default for Resolver {
    fn default() -> Self { Self::V1 }
}
