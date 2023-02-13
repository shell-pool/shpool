use std::os::unix::{
    io::FromRawFd,
    net::UnixListener,
};

use anyhow::{
    anyhow,
    Context,
};

/// activation_socket converts the systemd activation socket
/// to a usable UnixStream.
pub fn activation_socket() -> anyhow::Result<UnixListener> {
    let fds = systemd::daemon::listen_fds(true).context("getting listen_fds iterator")?;
    if fds.len() != 1 {
        return Err(anyhow!(
            "expected exactly 1 activation fd, got {}",
            fds.len()
        ));
    }
    let fd = fds.iter().next().ok_or(anyhow!("no fd"))?;

    let is_unix = systemd::daemon::is_socket(
        fd,
        Some(libc::AF_UNIX as u32),
        Some(systemd::daemon::SocketType::Stream),
        systemd::daemon::Listening::IsListening,
    )
    .context("checking if socket is unix socket")?;
    if !is_unix {
        return Err(anyhow!("expected to be passed a unix socket"));
    }

    // Saftey: we have just verified that this is a streaming
    //         unix socket with the systemd library.
    unsafe { Ok(UnixListener::from_raw_fd(fd)) }
}
