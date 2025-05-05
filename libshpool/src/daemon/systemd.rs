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

use std::{
    env,
    os::{
        fd::{OwnedFd, RawFd},
        unix::{io::FromRawFd, net::UnixListener},
    },
};

use anyhow::{anyhow, Context};
use nix::sys::stat;

// the fd that systemd uses for the first activation socket
// (0 through 2 are for the std streams)
// Reference https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html#
const FIRST_ACTIVATION_SOCKET_FD: RawFd = 3;

/// activation_socket converts the systemd activation socket
/// to a usable UnixStream.
pub fn activation_socket() -> anyhow::Result<UnixListener> {
    let num_activation_socks = env::var("LISTEN_FDS")
        .context("fetching LISTEN_FDS env var")?
        .parse::<isize>()
        .context("parsing LISTEN_FDS as int")?;
    if num_activation_socks != 1 {
        return Err(anyhow!("expected exactly 1 activation fd, got {}", num_activation_socks));
    }

    // Safety: we have just checked that there is 1 activation fd, which starts at
    // FIRST_ACTIVATION_SOCKET_FD. This FD can be closed by us.
    let fd = unsafe { OwnedFd::from_raw_fd(FIRST_ACTIVATION_SOCKET_FD) };

    let sock_stat = stat::fstat(&fd).context("stating activation sock")?;
    if !stat::SFlag::from_bits_truncate(sock_stat.st_mode).contains(stat::SFlag::S_IFSOCK) {
        return Err(anyhow!("expected to be passed a unix socket"));
    }

    Ok(fd.into())
}
