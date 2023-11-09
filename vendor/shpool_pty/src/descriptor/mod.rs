mod err;

use ::libc;

pub use self::err::DescriptorError;
use std::ffi::CStr;
use std::os::unix::io::RawFd;

pub unsafe trait Descriptor {
    /// If the descriptor has a valid fd, return it
    fn take_raw_fd(&mut self) -> Option<RawFd>;

    /// The constructor function `open` opens the path
    /// and returns the fd.
    fn open(path: &CStr,
            flag: libc::c_int,
            mode: Option<libc::c_int>)
            -> Result<RawFd, DescriptorError> {
        // Safety: we've just ensured that path is non-null and the
        // other params are valid by construction.
        unsafe {
            match libc::open(path.as_ptr().cast(), flag, mode.unwrap_or_default()) {
                -1 => Err(DescriptorError::OpenFail),
                fd => Ok(fd),
            }
        }
    }

    /// The function `close` leaves the fd.
    fn close(&mut self) -> Result<(), DescriptorError> {
        if let Some(fd) = self.take_raw_fd() {
            // Safety: we take the fd here, ensuring that it cannot
            // be used or closed again afterwards. N.B. this is only
            // safe because there is no way for a descriptor to hand
            // out the raw fd for it to be stored somewhere and then
            // used by external code.
            unsafe {
                match libc::close(fd) {
                    -1 => Err(DescriptorError::CloseFail),
                    _ => Ok(()),
                }
            }
        } else {
            Ok(())
        }
    }

    /// The destructor function `drop` call the method `close`
    /// and log if an error occurred.
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            log::warn!("error closing fd: {:?}", e);
        }
    }
}
