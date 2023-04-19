mod err;

use ::libc;

pub use self::err::DescriptorError;
use std::os::unix::io::{AsRawFd, RawFd};

pub trait Descriptor: AsRawFd {
    /// The constructor function `open` opens the path
    /// and returns the fd.
    fn open(path: *const libc::c_char,
            flag: libc::c_int,
            mode: Option<libc::c_int>)
            -> Result<RawFd, DescriptorError> {
        unsafe {
            match libc::open(path, flag, mode.unwrap_or_default()) {
                -1 => Err(DescriptorError::OpenFail),
                fd => Ok(fd),
            }
        }
    }

    /// The function `close` leaves the fd.
    fn close(&self) -> Result<(), DescriptorError> {
        unsafe {
            match libc::close(self.as_raw_fd()) {
                -1 => Err(DescriptorError::CloseFail),
                _ => Ok(()),
            }
        }
    }

    /// The destructor function `drop` call the method `close`
    /// and log if an error occurred.
    fn drop(&self) {
        if let Err(e) = self.close() {
            log::warn!("error closing fd {}: {:?}", self.as_raw_fd(), e);
        }
    }
}
