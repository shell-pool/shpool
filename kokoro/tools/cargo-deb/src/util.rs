use std::collections::BTreeSet;
use std::path::Path;

/// Get the filename from a path.
/// Note: Due to the way the Path type works the final component is returned
/// even if it looks like a directory, e.g. "/some/dir/" will return "dir"...
pub(crate) fn fname_from_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into()
}

#[cfg(test)]
pub(crate) use tests::is_path_file;

#[cfg(not(test))]
pub(crate) fn is_path_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
pub(crate) use tests::read_file_to_string;

#[cfg(not(test))]
pub(crate) fn read_file_to_string(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

#[cfg(test)]
pub(crate) use tests::read_file_to_bytes;

#[cfg(not(test))]
pub(crate) fn read_file_to_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

/// Create a HashMap from one or more key => value pairs in a single statement.
///
/// # Usage
///
/// Any types supported by HashMap for keys and values are supported:
///
/// ```rust,ignore
/// let mut one = std::collections::HashMap::new();
/// one.insert(1, 'a');
/// assert_eq!(one, map!{ 1 => 'a' });
///
/// let mut two = std::collections::HashMap::new();
/// two.insert("a", 1);
/// two.insert("b", 2);
/// assert_eq!(two, map!{ "a" => 1, "b" => 2 });
/// ```
///
/// Empty maps are not supported, attempting to create one will fail to compile:
/// ```compile_fail
/// let empty = std::collections::HashMap::new();
/// assert_eq!(empty, map!{ });
/// ```
///
/// # Provenance
///
/// From: https://stackoverflow.com/a/27582993
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

/// A trait for returning a String containing items separated by the given
/// separator.
pub(crate) trait MyJoin {
    fn join(&self, sep: &str) -> String;
}

/// Returns a String containing the hash set items joined together by the given
/// separator.
///
/// # Usage
///
/// ```text
/// let two: BTreeSet<String> = vec!["a", "b"].into_iter().map(|s| s.to_owned()).collect();
/// assert_eq!("ab", two.join(""));
/// assert_eq!("a,b", two.join(","));
/// ```
impl MyJoin for BTreeSet<String> {
    fn join(&self, sep: &str) -> String {
        self.iter().map(|item| item.as_str()).collect::<Vec<&str>>().join(sep)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use lazy_static::lazy_static;
    use std::collections::HashMap;

    lazy_static! {
        static ref ERROR_REGEX: regex::Regex = regex::Regex::new(r"^error:(?P<error_name>.+)$").unwrap();
    }

    // ---------------------------------------------------------------------
    // Begin: test virtual filesystem
    // ---------------------------------------------------------------------
    // The pkgfile() function accesses the filesystem directly via its use
    // the Path(Buf)::is_file() method which checks for the existence of a
    // file in the real filesystem.
    //
    // To test this without having to create real files and directories we
    // extend the PathBuf type via a trait with a mock_is_file() method
    // which, in test builds, is used by pkgfile() instead of the real
    // PathBuf::is_file() method.
    //
    // The mock_is_file() method looks up a path in a vector which
    // represents a set of paths in a virtual filesystem. However, accessing
    // global state in a multithreaded test run is unsafe, plus we want each
    // test to define its own virtual filesystem to test against, not a
    // single global virtual filesystem shared by all tests.
    //
    // This test specific virtual filesystem is implemented as a map,
    // protected by a thread local such that each test (thread) gets its own
    // instance. To be able to mutate the map it is wrapped inside a Mutex.
    // To make this setup easier to work with we define a few  helper
    // functions:
    //
    //   - add_test_fs_paths() - adds paths to the current tests virtual fs
    //   - set_test_fs_path_content() - set the file content (initially "")
    //   - with_test_fs() - passes the current tests virtual fs vector to
    //                      a user defined callback function.
    use std::sync::Mutex;

    pub(crate) struct TestPath {
        _filename: &'static str,
        contents: String,
        read_count: u16,
    }

    impl TestPath {
        fn new(filename: &'static str, contents: String) -> Self {
            TestPath {
                _filename: filename,
                contents,
                read_count: 0,
            }
        }

        fn read(&mut self) -> String {
            self.read_count += 1;
            self.contents.clone()
        }

        fn count(&self) -> u16 {
            self.read_count
        }
    }

    thread_local!(
        static MOCK_FS: Mutex<HashMap<&'static str, TestPath>> = Mutex::new(HashMap::new());
    );

    pub(crate) struct ResetFsGuard;

    impl Drop for ResetFsGuard {
        fn drop(&mut self) {
            MOCK_FS.with(|fs| {
                fs.lock().unwrap().clear();
            });
        }
    }

    #[must_use]
    pub(crate) fn add_test_fs_paths(paths: &[&'static str]) -> ResetFsGuard {
        MOCK_FS.with(|fs| {
            let mut fs_map = fs.lock().unwrap();
            for path in paths {
                fs_map.insert(path, TestPath::new(path, "".to_owned()));
            }
        });
        ResetFsGuard
    }

    pub(crate) fn set_test_fs_path_content(path: &'static str, contents: String) {
        MOCK_FS.with(|fs| {
            let mut fs_map = fs.lock().unwrap();
            fs_map.insert(path, TestPath::new(path, contents));
        })
    }

    fn with_test_fs<F, R>(callback: F) -> R
    where
        F: Fn(&mut HashMap<&'static str, TestPath>) -> R,
    {
        MOCK_FS.with(|fs| callback(&mut fs.lock().unwrap()))
    }

    pub(crate) fn is_path_file(path: &Path) -> bool {
        with_test_fs(|fs| fs.contains_key(&path.to_str().unwrap()))
    }

    pub(crate) fn get_read_count(path: &str) -> u16 {
        with_test_fs(|fs| fs.get(path).unwrap().count())
    }

    pub(crate) fn read_file_to_string(path: &Path) -> std::io::Result<String> {
        fn str_to_err(str: &str) -> std::io::Result<String> {
            Err(std::io::Error::from(match str {
                "InvalidInput"     => std::io::ErrorKind::InvalidInput,
                "Interrupted"      => std::io::ErrorKind::Interrupted,
                "PermissionDenied" => std::io::ErrorKind::PermissionDenied,
                "NotFound"         => std::io::ErrorKind::NotFound,
                "Other"            => std::io::ErrorKind::Other,
                _                  => panic!("Unknown I/O ErrorKind '{str}'")
            }))
        }

        with_test_fs(|fs| match fs.get_mut(path.to_str().unwrap()) {
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Test filesystem path {:?} does not exist", path),
            )),
            Some(test_path) => {
                let contents = test_path.read();
                match ERROR_REGEX.captures(&contents) {
                    None => Ok(contents),
                    Some(caps) => match caps.name("error_name") {
                        None => Ok(contents),
                        Some(re_match) => str_to_err(re_match.as_str()),
                    },
                }
            }
        })
    }

    pub(crate) fn read_file_to_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
        match read_file_to_string(path) {
            Ok(contents) => Ok(Vec::from(contents.as_bytes())),
            Err(x) => Err(x),
        }
    }

    // ---------------------------------------------------------------------
    // End: test virtual filesystem
    // ---------------------------------------------------------------------

    use super::*;

    #[test]
    fn fname_from_path_returns_file_name_even_if_file_does_not_exist() {
        assert_eq!("some_name", fname_from_path(Path::new("some_name")));
        assert_eq!("some_name", fname_from_path(Path::new("/some_name")));
        assert_eq!("some_name", fname_from_path(Path::new("/a/b/some_name")));
    }

    #[test]
    fn fname_from_path_returns_file_name_even_if_it_looks_like_a_directory() {
        assert_eq!("some_name", fname_from_path(Path::new("some_name/")));
    }

    #[test]
    #[should_panic]
    fn fname_from_path_panics_when_path_is_empty() {
        assert_eq!("", fname_from_path(Path::new("")));
    }

    #[test]
    #[should_panic]
    fn fname_from_path_panics_when_path_has_no_filename() {
        assert_eq!("", fname_from_path(Path::new("/a/")));
    }

    #[test]
    fn map_macro() {
        let mut one = std::collections::HashMap::new();
        one.insert(1, 'a');
        assert_eq!(one, map! { 1 => 'a' });

        let mut two = std::collections::HashMap::new();
        two.insert("a", 1);
        two.insert("b", 2);
        assert_eq!(two, map! { "a" => 1, "b" => 2 });
    }

    #[test]
    fn btreeset_join() {
        let empty: BTreeSet<String> = vec![].into_iter().collect();
        assert_eq!("", empty.join(""));
        assert_eq!("", empty.join(","));

        let one: BTreeSet<String> = vec!["a"].into_iter().map(|s| s.to_owned()).collect();
        assert_eq!("a", one.join(""));
        assert_eq!("a", one.join(","));

        let two: BTreeSet<String> = vec!["a", "b"].into_iter().map(|s| s.to_owned()).collect();
        assert_eq!("ab", two.join(""));
        assert_eq!("a,b", two.join(","));
    }
}
