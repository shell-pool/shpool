#![recursion_limit = "128"]

/*!

## Making deb packages

If you only want to make some `*.deb` files, and you're not a developer of tools
for Debian packaging, **[see `cargo deb` command usage described in the
README instead](https://github.com/kornelski/cargo-deb#readme)**.

```sh
cargo install cargo-deb
cargo deb # run this in your Cargo project directory
```

## Making tools for making deb packages

The library interface is experimental. See `main.rs` for usage.
*/

#[macro_use] extern crate quick_error;

pub mod compress;
pub mod control;
pub mod data;
pub mod listener;
pub mod manifest;
pub use crate::debarchive::DebArchive;
pub use crate::error::*;
pub use crate::manifest::Config;

#[macro_use]
mod util;
mod config;
mod debarchive;
mod dependencies;
mod dh_installsystemd;
mod dh_lib;
mod error;
mod ok_or;
mod pathbytes;
mod tararchive;
mod wordsplit;

use crate::listener::Listener;
use crate::manifest::AssetSource;
use rayon::prelude::*;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

const TAR_REJECTS_CUR_DIR: bool = true;

/// Set by `build.rs`
const DEFAULT_TARGET: &str = env!("CARGO_DEB_DEFAULT_TARGET");

/// Run `dpkg` to install `deb` archive at the given path
pub fn install_deb(path: &Path) -> CDResult<()> {
    let status = Command::new("sudo").arg("dpkg").arg("-i").arg(path)
        .status()?;
    if !status.success() {
        return Err(CargoDebError::InstallFailed);
    }
    Ok(())
}

/// Creates empty (removes files if needed) target/debian/foo directory so that we can start fresh.
pub fn reset_deb_temp_directory(options: &Config) -> io::Result<()> {
    let deb_dir = options.default_deb_output_dir();
    let deb_temp_dir = options.deb_temp_dir();
    remove_deb_temp_directory(options);
    // For backwards compatibility with previous cargo-deb behavior, also delete .deb from target/debian,
    // but this time only debs from other versions of the same package
    let g = deb_dir.join(DebArchive::filename_glob(options));
    if let Ok(old_files) = glob::glob(g.to_str().expect("utf8 path")) {
        for old_file in old_files.flatten() {
            let _ = fs::remove_file(old_file);
        }
    }
    fs::create_dir_all(deb_temp_dir)
}

/// Removes the target/debian/foo
pub fn remove_deb_temp_directory(options: &Config) {
    let deb_temp_dir = options.deb_temp_dir();
    let _ = fs::remove_dir(&deb_temp_dir);
}

/// Builds a binary with `cargo build`
pub fn cargo_build(options: &Config, target: Option<&str>, build_command: &str, build_flags: &[String], verbose: bool) -> CDResult<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&options.package_manifest_dir);
    cmd.arg(build_command);

    cmd.args(build_flags);

    if verbose {
        cmd.arg("--verbose");
    }
    if let Some(target) = target {
        cmd.args(["--target", target]);
        // Set helpful defaults for cross-compiling
        if env::var_os("PKG_CONFIG_ALLOW_CROSS").is_none() && env::var_os("PKG_CONFIG_PATH").is_none() {
            let pkg_config_path = format!("/usr/lib/{}/pkgconfig", debian_triple_from_rust_triple(target));
            if Path::new(&pkg_config_path).exists() {
                cmd.env("PKG_CONFIG_ALLOW_CROSS", "1");
                cmd.env("PKG_CONFIG_PATH", pkg_config_path);
            }
        }
    }
    if !options.default_features {
        cmd.arg("--no-default-features");
    }
    let features = &options.features;
    if !features.is_empty() {
        cmd.args(["--features", &features.join(",")]);
    }

    log::debug!("cargo build {:?}", cmd.get_args());

    let status = cmd.status()
        .map_err(|e| CargoDebError::CommandFailed(e, "cargo"))?;
    if !status.success() {
        return Err(CargoDebError::BuildFailed);
    }
    Ok(())
}

// Maps Rust's blah-unknown-linux-blah to Debian's blah-linux-blah. This is debian's multiarch.
fn debian_triple_from_rust_triple(rust_target_triple: &str) -> String {
    let mut p = rust_target_triple.split('-');
    let arch = p.next().unwrap();
    let abi = p.last().unwrap_or("");

    let (darch, dabi) = match (arch, abi) {
        ("i586", _) |
        ("i686", _) => ("i386", "gnu"),
        ("x86_64", _) => ("x86_64", "gnu"),
        ("aarch64", _) => ("aarch64", "gnu"),
        (arm, abi) if arm.starts_with("arm") || arm.starts_with("thumb") => {
            ("arm", if abi.ends_with("hf") {"gnueabihf"} else {"gnueabi"})
        },
        ("mipsel", _) => ("mipsel", "gnu"),
        (risc, _) if risc.starts_with("riscv64") => ("riscv64", "gnu"),
        (arch, abi) => (arch, abi),
    };
    format!("{darch}-linux-{dabi}")
}


/// Debianizes the architecture name. Weirdly, architecture and multiarch use different naming conventions in Debian!
pub(crate) fn debian_architecture_from_rust_triple(target: &str) -> &str {
    let mut parts = target.split('-');
    let arch = parts.next().unwrap();
    let abi = parts.last().unwrap_or("");
    match (arch, abi) {
        // https://wiki.debian.org/Multiarch/Tuples
        // rustc --print target-list
        // https://doc.rust-lang.org/std/env/consts/constant.ARCH.html
        ("aarch64", _) => "arm64",
        ("mips64", "gnuabin32") => "mipsn32",
        ("mips64el", "gnuabin32") => "mipsn32el",
        ("mipsisa32r6", _) => "mipsr6",
        ("mipsisa32r6el", _) => "mipsr6el",
        ("mipsisa64r6", "gnuabi64") => "mips64r6",
        ("mipsisa64r6", "gnuabin32") => "mipsn32r6",
        ("mipsisa64r6el", "gnuabi64") => "mips64r6el",
        ("mipsisa64r6el", "gnuabin32") => "mipsn32r6el",
        ("powerpc", "gnuspe") => "powerpcspe",
        ("powerpc64", _) => "ppc64",
        ("powerpc64le", _) => "ppc64el",
        ("riscv64gc", _) => "riscv64",
        ("i586", _) | ("i686", _) | ("x86", _) => "i386",
        ("x86_64", "gnux32") => "x32",
        ("x86_64", _) => "amd64",
        (arm, gnueabi) if arm.starts_with("arm") && gnueabi.ends_with("hf") => "armhf",
        (arm, _) if arm.starts_with("arm") => "armel",
        (other_arch, _) => other_arch,
    }
}
fn ensure_success(status: ExitStatus) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, status.to_string()))
    }
}

/// Strips the binary that was created with cargo
pub fn strip_binaries(options: &mut Config, target: Option<&str>, listener: &dyn Listener, separate_file: bool) -> CDResult<()> {
    let mut cargo_config = None;
    let objcopy_tmp;
    let strip_tmp;
    let mut objcopy_cmd = Path::new("objcopy");
    let mut strip_cmd = Path::new("strip");

    if let Some(target) = target {
        cargo_config = options.cargo_config()?;
        if let Some(ref conf) = cargo_config {
            if let Some(cmd) = conf.objcopy_command(target) {
                listener.info(format!("Using '{}' for '{target}'", cmd.display()));
                objcopy_tmp = cmd;
                objcopy_cmd = &objcopy_tmp;
            }

            if let Some(cmd) = conf.strip_command(target) {
                listener.info(format!("Using '{}' for '{target}'", cmd.display()));
                strip_tmp = cmd;
                strip_cmd = &strip_tmp;
            }
        }
    }

    let stripped_binaries_output_dir = options.default_deb_output_dir();

    options.built_binaries_mut().into_par_iter().enumerate()
        .filter(|(_, asset)| !asset.source.archive_as_symlink_only()) // data won't be included, so nothing to strip
        .try_for_each(|(i, asset)| {
        let new_source = match asset.source.path() {
            Some(path) => {
                if !path.exists() {
                    return Err(CargoDebError::StripFailed(path.to_owned(), "The file doesn't exist".into()));
                }

                // The debug_path and debug_filename should never return None if we have an AssetSource::Path
                let debug_path = asset.source.debug_source().expect("Failed to compute debug source path");
                let conf_path = cargo_config.as_ref().map(|c| c.path())
                    .unwrap_or_else(|| Path::new(".cargo/config"));

                if separate_file {
                    log::debug!("extracting debug info of {} with {}", path.display(), objcopy_cmd.display());
                    let _ = std::fs::remove_file(&debug_path);
                    Command::new(objcopy_cmd)
                        .arg("--only-keep-debug")
                        .arg(path)
                        .arg(&debug_path)
                        .status()
                        .and_then(ensure_success)
                        .map_err(|err| {
                            if let Some(target) = target {
                                CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] objcopy = {{ path =\"{}\" }} in {}", objcopy_cmd.display(), err, target, objcopy_cmd.display(), conf_path.display()))
                            } else {
                                CargoDebError::CommandFailed(err, "objcopy")
                            }
                        })?;
                }

                let file_name = path.file_name().ok_or(CargoDebError::Str("bad path"))?;
                let file_name = format!("{}.tmp{}-stripped", file_name.to_string_lossy(), i);
                let stripped_temp_path = stripped_binaries_output_dir.join(file_name);
                let _ = std::fs::remove_file(&stripped_temp_path);

                log::debug!("stripping {} with {}", path.display(), strip_cmd.display());
                Command::new(strip_cmd)
                   .arg("--strip-unneeded")
                   .arg("-o")
                   .arg(&stripped_temp_path)
                   .arg(path)
                   .status()
                   .and_then(ensure_success)
                   .map_err(|err| {
                        if let Some(target) = target {
                            CargoDebError::StripFailed(path.to_owned(), format!("{}: {}.\nhint: Target-specific strip commands are configured in [target.{}] strip = {{ path = \"{}\" }} in {}", strip_cmd.display(), err, target, strip_cmd.display(), conf_path.display()))
                        } else {
                            CargoDebError::CommandFailed(err, "strip")
                        }
                    })?;

                if !stripped_temp_path.exists() {
                    return Err(CargoDebError::StripFailed(path.to_owned(), format!("{} command failed to create output '{}'", strip_cmd.display(), stripped_temp_path.display())));
                }

                if separate_file {
                    log::debug!("linking debug info to {} with {}", debug_path.display(), objcopy_cmd.display());
                    let debug_filename = debug_path.file_name().expect("Built binary has no filename");
                    Command::new(objcopy_cmd)
                        .current_dir(debug_path.parent().expect("Debug source file had no parent path"))
                        .arg(format!(
                            "--add-gnu-debuglink={}",
                            debug_filename.to_str().expect("Debug source file had no filename")
                        ))
                        .arg(&stripped_temp_path)
                        .status()
                        .and_then(ensure_success)
                        .map_err(|err| CargoDebError::CommandFailed(err, "objcopy"))?;
                }
                listener.info(format!("Stripped '{}'", path.display()));
                AssetSource::Path(stripped_temp_path)
            },
            None => {
                // This is unexpected - emit a warning if we come across it
                listener.warning(format!("Found built asset with non-path source '{:?}'", asset));
                return Ok(());
            },
        };
        asset.source = new_source;
        Ok::<_, CargoDebError>(())
    })?;

    if separate_file {
        // If we want to debug symbols included in a separate file, add these files to the debian assets
        options.add_debug_assets();
    }

    Ok(())
}
