use std::{
    env,
    os::unix::{io::FromRawFd, net::UnixListener},
};

use anyhow::{anyhow, Context};
use nix::sys::stat;

// the fd that uses for the first activation socket (0 through 2 are for the std
// streams)
const FIRST_ACTIVATION_SOCKET_FD: i32 = 3;

/// activation_socket converts the systemd activation socket
/// to a usable UnixStream.
pub fn activation_socket() -> anyhow::Result<UnixListener> {
    let num_activation_socks = env::var("LISTEN_FDS")
        .context("fetching LISTEN_FDS env var")?
        .parse::<isize>()
        .context("parsing LISTEN_FDS as int")?;
    if num_activation_socks != 1 {
        return Err(anyhow!("expected exactly 1 activation fd, got {}", num_activation_socks,));
    }

    let fd = FIRST_ACTIVATION_SOCKET_FD;
    let sock_stat = stat::fstat(fd).context("stating activation sock")?;
    if !stat::SFlag::from_bits_truncate(sock_stat.st_mode).contains(stat::SFlag::S_IFSOCK) {
        return Err(anyhow!("expected to be passed a unix socket"));
    }

    // Saftey: we have just verified that this is a unix socket.
    unsafe { Ok(UnixListener::from_raw_fd(fd)) }
}
