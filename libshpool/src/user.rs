// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{ffi::CStr, io, ptr};

use anyhow::anyhow;

#[derive(Debug)]
pub struct Info {
    pub default_shell: String,
    pub home_dir: String,
    pub user: String,
}

pub fn info() -> anyhow::Result<Info> {
    let mut passwd_str_buf: [libc::c_char; 1024 * 4] = [0; 1024 * 4];
    let mut passwd = libc::passwd {
        pw_name: ptr::null_mut(),
        pw_passwd: ptr::null_mut(),
        pw_uid: 0,
        pw_gid: 0,
        pw_gecos: ptr::null_mut(),
        pw_dir: ptr::null_mut(),
        pw_shell: ptr::null_mut(),
    };
    let mut passwd_res_ptr: *mut libc::passwd = ptr::null_mut();
    unsafe {
        // Safety: pretty much pure ffi, passwd and passwd_str_buf correctly
        //         have memory backing them.
        let errno = libc::getpwuid_r(
            libc::getuid(),
            &mut passwd,
            passwd_str_buf.as_mut_ptr(),
            passwd_str_buf.len(),
            &mut passwd_res_ptr as *mut *mut libc::passwd,
        );
        if passwd_res_ptr.is_null() {
            if errno == 0 {
                return Err(anyhow!("could not find current user, should be impossible"));
            } else {
                return Err(anyhow!(
                    "error resolving user info: {}",
                    io::Error::from_raw_os_error(errno)
                ));
            }
        }

        // Safety: these pointers are all cstrings
        Ok(Info {
            default_shell: String::from(String::from_utf8_lossy(
                CStr::from_ptr(passwd.pw_shell).to_bytes(),
            )),
            home_dir: String::from(String::from_utf8_lossy(
                CStr::from_ptr(passwd.pw_dir).to_bytes(),
            )),
            user: String::from(String::from_utf8_lossy(CStr::from_ptr(passwd.pw_name).to_bytes())),
        })
    }
}
