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

use std::ffi::CStr;

use anyhow::anyhow;

#[derive(Debug)]
pub struct Info {
    pub default_shell: String,
    pub home_dir: String,
    pub user: String,
}

pub fn info() -> anyhow::Result<Info> {
    // Safety: we immediately copy the data into an owned buffer and don't
    //         use it subsequently.
    unsafe {
        *libc::__errno_location() = 0;
        let passwd = libc::getpwuid(libc::getuid());
        let errno = nix::errno::errno();
        if errno != 0 {
            return Err(anyhow!("error getting passwd: {:?}", nix::errno::from_i32(errno)));
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
