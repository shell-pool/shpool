use std::ffi::CStr;

use anyhow::anyhow;

#[derive(Debug)]
pub struct Info {
    pub default_shell: String,
    pub home_dir: String,
    pub user: String,
}

pub fn info() -> anyhow::Result<Info> {
    // Saftey: we immediately copy the data into an owned buffer and don't
    //         use it subsequently.
    unsafe {
        *libc::__errno_location() = 0;
        let passwd = libc::getpwuid(libc::getuid());
        let errno = nix::errno::errno();
        if errno != 0 {
            return Err(anyhow!(
                "error getting passwd: {:?}",
                nix::errno::from_i32(errno)
            ));
        }

        Ok(Info {
            default_shell: String::from(String::from_utf8_lossy(
                CStr::from_ptr((*passwd).pw_shell).to_bytes(),
            )),
            home_dir: String::from(String::from_utf8_lossy(
                CStr::from_ptr((*passwd).pw_dir).to_bytes(),
            )),
            user: String::from(String::from_utf8_lossy(
                CStr::from_ptr((*passwd).pw_name).to_bytes(),
            )),
        })
    }
}
