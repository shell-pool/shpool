use std::path::PathBuf;
use std::collections::HashSet;
use std::fs::read_dir;
use std::io;
use std::path::Path;

pub trait AbstractFilesystem {
    /// List all files and directories at the given relative path (no leading `/`).
    fn file_names_in(&self, rel_path: &str) -> io::Result<HashSet<Box<str>>>;

    /// The `rel_path_hint` may be specified explicitly by `package.workspace` (it may be relative like `"../", without `Cargo.toml`) or `None`,
    /// which means you have to search for workspace's `Cargo.toml` in parent directories.
    ///
    /// Read bytes of the root workspace manifest TOML file and return the path it's been read from,
    /// preferably an absolute path (it will be used as the base path for inherited readmes).
    fn read_root_workspace(&self, _rel_path_hint: Option<&str>) -> io::Result<(Vec<u8>, PathBuf)> {
        Err(io::ErrorKind::NotFound.into())
    }
}

impl<T> AbstractFilesystem for &T
where
    T: AbstractFilesystem + ?Sized,
{
    fn file_names_in(&self, rel_path: &str) -> io::Result<HashSet<Box<str>>> {
        <T as AbstractFilesystem>::file_names_in(*self, rel_path)
    }
}

pub struct Filesystem<'a> {
    path: &'a Path,
}

impl<'a> Filesystem<'a> {
    #[must_use] pub fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl<'a> AbstractFilesystem for Filesystem<'a> {
    fn file_names_in(&self, rel_path: &str) -> io::Result<HashSet<Box<str>>> {
        Ok(read_dir(self.path.join(rel_path))?.filter_map(|entry| {
            entry.ok().map(|e| {
                e.file_name().to_string_lossy().into_owned().into()
            })
        })
        .collect())
    }

    fn read_root_workspace(&self, path: Option<&str>) -> io::Result<(Vec<u8>, PathBuf)> {
        match path {
            Some(path) => {
                let ws = self.path.join(path);
                Ok((std::fs::read(ws.join("Cargo.toml"))?, ws))
            },
            None => {
                // Try relative path first
                match find_in(self.path) {
                    Ok(found) => Ok(found),
                    Err(err) if self.path.is_absolute() => Err(err),
                    Err(_) => {
                        find_in(&self.path.ancestors().last().unwrap().canonicalize()?)
                    }
                }
            }
        }
    }
}

/// This doesn't check if the `Cargo.toml` is just a nested package, not a workspace.
/// If you run into this problem: use `cargo_metadata` to find the workspace properly,
/// or move the decoy package to a subdirectory.
fn find_in(path: &Path) -> io::Result<(Vec<u8>, PathBuf)> {
    path.ancestors().skip(1)
        .map(|parent| parent.join("Cargo.toml"))
        .find_map(|p| {
            Some((std::fs::read(&p).ok()?, p))
        })
        .ok_or(io::ErrorKind::NotFound.into())
}
