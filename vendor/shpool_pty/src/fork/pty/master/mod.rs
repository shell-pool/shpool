mod err;

use libc;

use ::descriptor::Descriptor;

pub use self::err::{MasterError, Result};
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

#[derive(Debug, Copy, Clone)]
pub struct Master {
    pty: RawFd,
}

impl Master {
    pub fn new(path: *const ::libc::c_char) -> Result<Self> {
        match Self::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(MasterError::BadDescriptor(cause)),
            Ok(fd) => Ok(Master { pty: fd }),
        }
    }

    /// Change UID and GID of slave pty associated with master pty whose
    /// fd is provided, to the real UID and real GID of the calling thread.
    pub fn grantpt(&self) -> Result<libc::c_int> {
        unsafe {
            match libc::grantpt(self.as_raw_fd()) {
                -1 => Err(MasterError::GrantptError),
                c => Ok(c),
            }
        }
    }

    /// Unlock the slave pty associated with the master to which fd refers.
    pub fn unlockpt(&self) -> Result<libc::c_int> {
        unsafe {
            match libc::unlockpt(self.as_raw_fd()) {
                -1 => Err(MasterError::UnlockptError),
                c => Ok(c),
            }
        }
    }

    /// Returns a pointer to a static buffer, which will be overwritten on
    /// subsequent calls.
    pub fn ptsname(&self) -> Result<*const libc::c_char> {
        unsafe {
            match libc::ptsname(self.as_raw_fd()) {
                c if c.is_null() => Err(MasterError::PtsnameError),
                c => Ok(c),
            }
        }
    }
}

impl Descriptor for Master {}

impl AsRawFd for Master {
    /// The accessor function `as_raw_fd` returns the fd.
    fn as_raw_fd(&self) -> RawFd {
        self.pty
    }
}

impl io::Read for Master {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            match libc::read(self.as_raw_fd(),
                             buf.as_mut_ptr() as *mut libc::c_void,
                             buf.len()) {
                -1 => Ok(0),
                len => Ok(len as usize),
            }
        }
    }
}

impl io::Write for Master {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            match libc::write(self.as_raw_fd(),
                              buf.as_ptr() as *const libc::c_void,
                              buf.len()) {
                -1 => Err(io::Error::last_os_error()),
                ret => Ok(ret as usize),
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
