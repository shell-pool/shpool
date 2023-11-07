mod err;

use libc;

use ::descriptor::Descriptor;

pub use self::err::{MasterError, Result};
use std::io;
use std::os::unix::io::RawFd;

#[derive(Debug, Copy, Clone)]
pub struct Master {
    pty: Option<RawFd>,
}

impl Master {
    pub fn new(path: *const ::libc::c_char) -> Result<Self> {
        match Self::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(MasterError::BadDescriptor(cause)),
            Ok(fd) => Ok(Master { pty: Some(fd) }),
        }
    }

    /// Extract the raw fd from the underlying object
    pub fn raw_fd(&self) -> &Option<RawFd> {
        &self.pty
    }

    /// Change UID and GID of slave pty associated with master pty whose
    /// fd is provided, to the real UID and real GID of the calling thread.
    pub fn grantpt(&self) -> Result<libc::c_int> {
        if let Some(fd) = self.pty {
            unsafe {
                match libc::grantpt(fd) {
                    -1 => Err(MasterError::GrantptError),
                    c => Ok(c),
                }
            }
        } else {
            Err(MasterError::NoFdError)
        }
    }

    /// Unlock the slave pty associated with the master to which fd refers.
    pub fn unlockpt(&self) -> Result<libc::c_int> {
        if let Some(fd) = self.pty {
            unsafe {
                match libc::unlockpt(fd) {
                    -1 => Err(MasterError::UnlockptError),
                    c => Ok(c),
                }
            }
        } else {
            Err(MasterError::NoFdError)
        }
    }

    /// Returns a pointer to a static buffer, which will be overwritten on
    /// subsequent calls.
    pub fn ptsname_r(&self, buf: &mut Vec<u8>) -> Result<()> {
        if let Some(fd) = self.pty {
            // Safety: the vector's memory is valid for the duration
            // of the call
            unsafe {
                let data: *mut u8 = &mut buf[0];
                match libc::ptsname_r(fd, data as *mut libc::c_char, buf.len()) {
                    0 => Ok(()),
                    _ => Err(MasterError::PtsnameError),  // should probably capture errno
                }
            }
        } else {
            Err(MasterError::NoFdError)
        }
    }
}

impl Descriptor for Master {
    fn take_raw_fd(&mut self) -> Option<RawFd> {
        self.pty.take()
    }
}

impl io::Read for Master {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(fd) = self.pty {
            unsafe {
                match libc::read(fd,
                                 buf.as_mut_ptr() as *mut libc::c_void,
                                 buf.len()) {
                    -1 => Ok(0),
                    len => Ok(len as usize),
                }
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "already closed"))
        }
    }
}

impl io::Write for Master {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(fd) = self.pty {
            unsafe {
                match libc::write(fd,
                                  buf.as_ptr() as *const libc::c_void,
                                  buf.len()) {
                    -1 => Err(io::Error::last_os_error()),
                    ret => Ok(ret as usize),
                }
            }
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "already closed"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
