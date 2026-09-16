// Copyright 2026 Google LLC
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

use anyhow::{anyhow, Context};
use nix::unistd;
use std::{os::unix::net::UnixStream, path::PathBuf};
use tracing::warn;

/// check makes sure that a process dialing in on the shpool sockets
/// has the same UID as the current user and that both have the same executable
/// path. Returns the peer's pid, which the kernel captures at connect time and
/// keeps fixed for the life of the connection.
#[cfg(target_os = "linux")]
pub(crate) fn check(sock: &UnixStream) -> anyhow::Result<libc::pid_t> {
    use nix::sys::socket;

    let peer_creds = socket::getsockopt(sock, socket::sockopt::PeerCredentials)
        .context("could not get peer creds from socket")?;
    let peer_uid = unistd::Uid::from_raw(peer_creds.uid());
    let self_uid = unistd::Uid::current();
    if peer_uid != self_uid {
        return Err(anyhow!("shpool prohibits connections across users"));
    }

    let peer_pid = unistd::Pid::from_raw(peer_creds.pid());
    let self_pid = unistd::Pid::this();
    let peer_exe = exe_for_pid(peer_pid).context("could not resolve exe from the pid")?;
    let self_exe = exe_for_pid(self_pid).context("could not resolve our own exe")?;
    if peer_exe != self_exe {
        warn!("attach binary differs from daemon binary");
    }

    Ok(peer_creds.pid())
}

#[cfg(target_os = "macos")]
pub(crate) fn check(sock: &UnixStream) -> anyhow::Result<libc::pid_t> {
    use std::{io, os::unix::io::AsRawFd};

    let mut peer_uid: libc::uid_t = 0;
    let mut peer_gid: libc::gid_t = 0;
    // Safety: getpeereid is standard BSD FFI, all pointers are valid
    unsafe {
        if libc::getpeereid(sock.as_raw_fd(), &mut peer_uid, &mut peer_gid) != 0 {
            return Err(anyhow!(
                "could not get peer uid from socket: {}",
                io::Error::last_os_error()
            ));
        }
    }
    let peer_uid = unistd::Uid::from_raw(peer_uid);
    let self_uid = unistd::Uid::current();
    if peer_uid != self_uid {
        return Err(anyhow!("shpool prohibits connections across users"));
    }

    let mut peer_pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // Safety: getsockopt is standard POSIX FFI, all pointers and sizes are
    // valid
    unsafe {
        if libc::getsockopt(
            sock.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            &mut peer_pid as *mut _ as *mut libc::c_void,
            &mut len,
        ) != 0
        {
            return Err(anyhow!(
                "could not get peer pid from socket: {}",
                io::Error::last_os_error()
            ));
        }
    }

    let self_pid = unistd::Pid::this();
    let peer_exe = exe_for_pid(unistd::Pid::from_raw(peer_pid))
        .context("could not resolve exe from the pid")?;
    let self_exe = exe_for_pid(self_pid).context("could not resolve our own exe")?;
    if peer_exe != self_exe {
        warn!("attach binary differs from daemon binary");
    }

    Ok(peer_pid)
}

#[cfg(target_os = "linux")]
fn exe_for_pid(pid: unistd::Pid) -> anyhow::Result<PathBuf> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe"))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn exe_for_pid(pid: unistd::Pid) -> anyhow::Result<PathBuf> {
    use libproc::proc_pid::pidpath;
    let path = pidpath(pid.as_raw())
        .map_err(|e| anyhow!("could not get exe path for pid {}: {:?}", pid, e))?;
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt as _, net::UnixListener},
        process::Command,
    };

    // Note: the cross-user branch is not covered here. Producing a connection
    // from a different uid requires privileges the test suite cannot count on
    // having, so `shpool prohibits connections across users` is exercised only
    // by inspection.

    // Both ends of a socketpair live in this process, so the peer is
    // trivially the same user running the same binary.
    #[test]
    fn check_accepts_a_peer_in_our_own_process() {
        let (ours, _theirs) = UnixStream::pair().expect("making a socketpair");

        let peer_pid = check(&ours).expect("our own process should pass the peer check");

        assert_eq!(peer_pid, unistd::getpid().as_raw());
    }

    // A peer that is already gone by the time we look at it gets rejected
    // rather than waved through. The two platforms notice at different points:
    // Linux still hands back the pid the kernel recorded at connect time and
    // we fail resolving its exe, while macOS refuses to report the pid at all
    // once the peer end is gone (ENOTCONN).
    #[cfg(target_os = "linux")]
    const GONE_PEER_ERROR: &str = "could not resolve exe from the pid";
    #[cfg(target_os = "macos")]
    const GONE_PEER_ERROR: &str = "could not get peer pid from socket";

    #[test]
    fn check_rejects_a_peer_that_exited_before_the_check() {
        let dir = tempfile::tempdir().expect("making a tmp dir");
        let sock_path = dir.path().join("peer.socket");
        let listener = UnixListener::bind(&sock_path).expect("binding the test socket");

        // Build the sockaddr before forking so the child only has to make
        // async-signal-safe libc calls.
        let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let path = CString::new(sock_path.as_os_str().as_bytes()).expect("path with no nul");
        let path = path.as_bytes_with_nul();
        assert!(path.len() <= addr.sun_path.len(), "test socket path is too long for sockaddr_un");
        for (slot, byte) in addr.sun_path.iter_mut().zip(path) {
            *slot = *byte as libc::c_char;
        }

        // Safety: the child only calls async-signal-safe libc functions before
        // _exit()ing, so it never takes the allocator lock or runs a
        // destructor belonging to the test harness.
        match unsafe { unistd::fork() }.expect("forking a peer") {
            unistd::ForkResult::Child => unsafe {
                let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
                let connected = fd >= 0
                    && libc::connect(
                        fd,
                        &addr as *const _ as *const libc::sockaddr,
                        std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
                    ) == 0;
                libc::_exit(if connected { 0 } else { 1 });
            },
            unistd::ForkResult::Parent { child } => {
                let status = nix::sys::wait::waitpid(child, None).expect("reaping the peer");
                assert_eq!(
                    status,
                    nix::sys::wait::WaitStatus::Exited(child, 0),
                    "the peer should have connected and exited cleanly"
                );

                // The kernel completed the connection at connect time, so it
                // is still sitting in the backlog even though the process
                // behind it is gone.
                let (stream, _addr) = listener.accept().expect("accepting the dead peer");

                let err = check(&stream).expect_err("a peer that is gone should be rejected");
                assert!(format!("{err:?}").contains(GONE_PEER_ERROR), "unexpected error: {err:?}");
            }
        }
    }

    #[test]
    fn exe_for_pid_resolves_our_own_exe() {
        let exe = exe_for_pid(unistd::Pid::this()).expect("resolving our own exe");

        assert_eq!(exe, std::env::current_exe().expect("looking up our own exe"));
    }

    // The peer is normally a different process, so exercise the cross-process
    // lookup too, along with the failure we get once the pid goes away.
    #[test]
    fn exe_for_pid_resolves_a_different_process() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().expect("spawning /bin/sleep");
        let pid = unistd::Pid::from_raw(child.id() as libc::pid_t);

        let exe = exe_for_pid(pid).expect("resolving the child's exe");
        assert_eq!(exe, std::fs::canonicalize("/bin/sleep").expect("canonicalizing /bin/sleep"));

        child.kill().expect("killing the child");
        child.wait().expect("reaping the child");

        exe_for_pid(pid).expect_err("a reaped pid should not resolve to an exe");
    }
}
