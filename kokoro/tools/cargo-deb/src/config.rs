use crate::error::*;
use std::borrow::Cow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CargoConfig {
    path: PathBuf,
    config: toml::Value,
}

impl CargoConfig {
    pub fn new<P: AsRef<Path>>(project_path: P) -> CDResult<Option<Self>> {
        Self::new_(project_path.as_ref())
    }

    #[allow(deprecated)]
    fn new_(project_path: &Path) -> CDResult<Option<Self>> {
        let mut project_path = project_path;
        loop {
            if let Some(conf) = Self::try_parse(project_path)? {
                return Ok(Some(conf));
            }
            if let Some(parent) = project_path.parent() {
                project_path = parent;
            } else {
                break;
            }
        }
        if let Some(home) = env::home_dir() {
            if let Some(conf) = Self::try_parse(&home)? {
                return Ok(Some(conf));
            }
        }
        if let Some(conf) = Self::try_parse(Path::new("/etc"))? {
            return Ok(Some(conf));
        }
        Ok(None)
    }

    fn try_parse(dir_path: &Path) -> CDResult<Option<Self>> {
        let mut path = dir_path.join(".cargo/config.toml");
        if !path.exists() {
            path = dir_path.join(".cargo/config");
            if !path.exists() {
                return Ok(None);
            }
        }
        Ok(Some(Self::from_str(&fs::read_to_string(&path)?, path)?))
    }

    fn from_str(input: &str, path: PathBuf) -> CDResult<Self> {
        let config = toml::from_str(input)?;
        Ok(CargoConfig { path, config })
    }

    fn target_conf(&self, target_triple: &str) -> Option<&toml::value::Table> {
        if let Some(target) = self.config.get("target").and_then(|t| t.as_table()) {
            return target.get(target_triple).and_then(|t| t.as_table());
        }
        None
    }

    pub fn strip_command(&self, target_triple: &str) -> Option<Cow<'_, Path>> {
        self.target_specific_command("strip", target_triple)
    }

    fn target_specific_command(&self, command_name: &str, target_triple: &str) -> Option<Cow<'_, Path>> {
        if let Some(target) = self.target_conf(target_triple) {
            let strip_config = target.get(command_name).and_then(|top| {
                let as_obj = top.get("path").and_then(|s| s.as_str());
                top.as_str().or(as_obj)
            });
            if let Some(strip) = strip_config {
                return Some(Cow::Borrowed(Path::new(strip)));
            }
        }

        let debian_target_triple = crate::debian_triple_from_rust_triple(target_triple);
        if let Some(linker) = self.linker_command(target_triple) {
            if linker.parent().is_some() {
                let linker_file_name = linker.file_name().unwrap().to_str().unwrap();
                // checks whether it's `/usr/bin/triple-ld` or `/custom-toolchain/ld`
                let strip_path = if linker_file_name.starts_with(&debian_target_triple) {
                    linker.with_file_name(format!("{}-{}", debian_target_triple, command_name))
                } else {
                    linker.with_file_name(command_name)
                };
                if strip_path.exists() {
                    return Some(strip_path.into());
                }
            }
        }
        let path = PathBuf::from(format!("/usr/bin/{debian_target_triple}-{command_name}"));
        if path.exists() {
            return Some(path.into());
        }
        None
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn linker_command(&self, target_triple: &str) -> Option<&Path> {
        if let Some(target) = self.target_conf(target_triple) {
            return target.get("linker").and_then(|l| l.as_str()).map(Path::new);
        }
        None
    }

    pub fn objcopy_command(&self, target_triple: &str) -> Option<Cow<'_, Path>> {
        if let Some(cmd) = self.target_specific_command("objcopy", target_triple) {
            return Some(cmd);
        }
        None
    }
}

#[test]
fn parse_strip() {
    let c = CargoConfig::from_str(r#"
[target.i686-unknown-dragonfly]
linker = "magic-ld"
strip = "magic-strip"

[target.'foo']
strip = { path = "strip2" }
"#, ".".into()).unwrap();

    assert_eq!("magic-strip", c.strip_command("i686-unknown-dragonfly").unwrap().as_os_str());
    assert_eq!("strip2", c.strip_command("foo").unwrap().as_os_str());
    assert_eq!(None, c.strip_command("bar"));
}

#[test]
fn parse_objcopy() {
    let c = CargoConfig::from_str(r#"
[target.i686-unknown-dragonfly]
linker = "magic-ld"
objcopy = "magic-objcopy"

[target.'foo']
objcopy = { path = "objcopy2" }
"#, ".".into()).unwrap();

    assert_eq!("magic-objcopy", c.objcopy_command("i686-unknown-dragonfly").unwrap().as_os_str());
    assert_eq!("objcopy2", c.objcopy_command("foo").unwrap().as_os_str());
    assert_eq!(None, c.objcopy_command("bar"));
}
