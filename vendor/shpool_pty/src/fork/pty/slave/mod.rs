mod err;

use ::descriptor::Descriptor;
use ::libc;

pub use self::err::{SlaveError, Result};
use std::os::unix::io::{AsRawFd, RawFd};

#[derive(Debug, Clone)]
pub struct Slave {
    pty: RawFd,
}

impl Slave {
    /// The constructor function `new` returns the Slave interface.
    pub fn new(path: *const ::libc::c_char) -> Result<Self> {
        match Self::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(SlaveError::BadDescriptor(cause)),
            Ok(fd) => Ok(Slave { pty: fd }),
        }
    }

    pub fn dup2(&self, std: libc::c_int) -> Result<libc::c_int> {
        unsafe {
            match libc::dup2(self.as_raw_fd(), std) {
                -1 => Err(SlaveError::Dup2Error),
                d => Ok(d),
            }
        }
    }
}

impl Descriptor for Slave {}

impl AsRawFd for Slave {
    /// The accessor function `as_raw_fd` returns the fd.
    fn as_raw_fd(&self) -> RawFd {
        self.pty
    }
}

impl Drop for Slave {
    fn drop(&mut self) {
        Descriptor::drop(self);
    }
}
