use std::borrow::Cow;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub trait AsUnixPathBytes {
    fn as_unix_path(&self) -> Cow<'_, [u8]>;
    fn to_bytes(&self) -> &[u8];
}

impl AsUnixPathBytes for Path {
    #[cfg(not(unix))]
    fn as_unix_path(&self) -> Cow<[u8]> {
        use std::path::Component::*;

        let parts: Vec<_> = self
            .components()
            .filter_map(|c| match c {
                Normal(c) => Some(c.to_str().expect("paths must be UTF-8").as_bytes()),
                RootDir => Some(&b"/"[..]),
                _ => None,
            })
            .collect();
        parts.join(&b'/').into()
    }

    #[cfg(not(unix))]
    fn to_bytes(&self) -> &[u8] {
        self.to_str().unwrap().as_bytes()
    }

    #[cfg(unix)]
    fn as_unix_path(&self) -> Cow<'_, [u8]> {
        self.as_os_str().as_bytes().into()
    }

    #[cfg(unix)]
    fn to_bytes(&self) -> &[u8] {
        self.as_os_str().as_bytes()
    }
}

#[test]
fn unix_path() {
    assert_eq!(b"foo/bar/baz"[..], Path::new("foo/bar/baz").as_unix_path()[..]);
}
