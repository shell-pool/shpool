// Copyright 2024 Google LLC
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
    ffi::OsStr,
    fs,
    io::{Seek, SeekFrom, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process, thread,
    time::Duration,
};

use crate::{config, consts, Args};

use anyhow::{anyhow, Context};
use nix::{
    fcntl,
    fcntl::FlockArg,
    sys::{wait, wait::WaitStatus},
    unistd,
    unistd::ForkResult,
};
use tracing::{error, info};

/// Check if we can connect to the control socket, and if we
/// can't, launch the daemon in the background. This should be
/// called by client subcommands like `shpool attach` or `shpool list`
pub fn maybe_launch_daemon<B, P>(
    config_manager: &config::Manager,
    args: &Args,
    shpool_bin: B,
    control_sock: P,
) -> anyhow::Result<()>
where
    B: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let control_sock = control_sock.as_ref();

    match UnixStream::connect(control_sock) {
        Ok(_) => {
            info!("daemon already running on {:?}, no need to autodaemonize", control_sock);
            // There is already a daemon listening on the control socket, we
            // don't need to do anything.
            return Ok(());
        }
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // The socket file exists but nothing is listening (stale socket).
            // Remove it so the new daemon can bind successfully.
            info!("stale socket at {:?}, removing before autodaemonizing", control_sock);
            std::fs::remove_file(control_sock)
                .with_context(|| format!("removing stale socket at {:?}", control_sock))?;
        }
        Err(_) => {
            // Socket file does not exist or other error; fall through to spawn
            // daemon.
        }
    }
    info!("no daemon running on {:?}, autodaemonizing", control_sock);

    let log_file = control_sock.with_file_name("daemonized-shpool.log");

    let mut cmd = process::Command::new(shpool_bin);
    if let Some(config_file) = &args.config_file {
        cmd.arg("--config-file").arg(config_file);
    }
    cmd.arg("--log-file")
        .arg(log_file)
        .arg("--socket")
        .arg(control_sock.as_os_str())
        .arg("daemon")
        .env(consts::AUTODAEMONIZE_VAR, "true")
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .spawn()
        .context("launching background daemon")?;
    info!("launched background daemon");

    // Now poll with exponential backoff until we can dial the control socket.
    if config_manager.get().nodaemonize_timeout.unwrap_or(false) {
        info!("waiting for daemon to come up with no timeout");
        let mut sleep_ms = 10;
        let max_sleep_ms = 2000;
        loop {
            if UnixStream::connect(control_sock).is_ok() {
                info!("connected to freshly launched background daemon");
                return Ok(());
            }

            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms *= 2;
            if sleep_ms > max_sleep_ms {
                sleep_ms = max_sleep_ms;
            }
        }
    } else {
        info!("waiting for daemon to come up with timeout");
        // `sum(10*(2**x) for x in range(9))` = 5110 ms = ~5 s
        let mut sleep_ms = 10;
        for _ in 0..9 {
            if UnixStream::connect(control_sock).is_ok() {
                info!("connected to freshly launched background daemon");
                return Ok(());
            }

            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms *= 2;
        }
    }

    Err(anyhow!("daemonizing: launched daemon, but control socket never came up"))
}

pub struct PidFileGuard {
    p: PathBuf,
}

impl PidFileGuard {
    pub fn path(&self) -> &PathBuf {
        &self.p
    }
}

impl std::ops::Drop for PidFileGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.p) {
            error!("cleaning up pid file: {:?}", e);
        }
    }
}

/// Perform the traditional daemonization double-fork setsid dance.
/// This should be called from within `shpool daemon` to detach it
/// from the launching shell.
///
/// Safety: see nix::unistd::fork for preconditions.
pub unsafe fn daemonize(pid_path: PathBuf) -> anyhow::Result<PidFileGuard> {
    let old_mask = nix::sys::stat::umask(nix::sys::stat::Mode::empty());
    info!("set empty umask (old mask: {:?}", old_mask);

    // `cd /` in order to avoid holding open the directory the user
    // happened to launch `shpool attach` in so that we don't block
    // deletes of that directory the whole time that the daemon
    // sticks around.
    std::env::set_current_dir("/").context("cding to root")?;

    // Fork and become the child in order to stop being the process group
    // leader. We need to avoid being the process group leader in order
    // to call setsid().
    //
    // Safety: the caller has ensured fork preconditions are met, which
    // meet the become_child preconditions.
    unsafe { become_child(true) }.context("first fork")?;

    let sid = unistd::setsid().context("creating new session")?;
    info!("while daemonizing setsid() = {}", sid);

    let pid_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&pid_path)
        .context("opening pid file")?;
    let mut pid_file = match fcntl::Flock::lock(pid_file, FlockArg::LockExclusiveNonblock) {
        Ok(l) => l,
        Err((_, errno)) if errno == nix::errno::Errno::EWOULDBLOCK => {
            return Err(anyhow!("another daemon is already running"));
        }
        Err((_, errno)) => {
            return Err(anyhow!("locking pid file: {:?}", errno));
        }
    };

    // Safety: the caller has ensured fork preconditions are met, which
    // meet the become_child preconditions.
    unsafe { become_child(false) }.context("second fork")?;

    // Actually write the pid file contents only now that we are in
    // the grandchild and have our final pid.
    pid_file.set_len(0).context("truncating pid file")?;
    pid_file.seek(SeekFrom::Start(0)).context("seeking to start of pid file")?;
    writeln!(*pid_file, "{}", std::process::id()).context("writing pid")?;

    redirect_std_fds_to_null()?;

    Ok(PidFileGuard { p: pid_path })
}

fn redirect_std_fds_to_null() -> anyhow::Result<()> {
    // Safety: path is a valid null terminated string, O_RDWR is a valid flag.
    let nullfd = unsafe { libc::open(b"/dev/null\0" as *const [u8; 10] as _, libc::O_RDWR) };
    if nullfd == -1 {
        return Err(anyhow!("opening /dev/null: {}", nix::errno::Errno::last()));
    }
    // Safety: nullfd is valid and newfd need not be open for saftey
    let fd = unsafe { libc::dup2(nullfd, libc::STDIN_FILENO) };
    if fd == -1 {
        return Err(anyhow!("redirecting stdin to /dev/null: {}", nix::errno::Errno::last()));
    }
    // Safety: nullfd is valid and newfd need not be open for saftey
    let fd = unsafe { libc::dup2(nullfd, libc::STDOUT_FILENO) };
    if fd == -1 {
        return Err(anyhow!("redirecting stdout to /dev/null: {}", nix::errno::Errno::last()));
    }
    // Safety: nullfd is valid and newfd need not be open for saftey
    let fd = unsafe { libc::dup2(nullfd, libc::STDERR_FILENO) };
    if fd == -1 {
        return Err(anyhow!("redirecting stderr to /dev/null: {}", nix::errno::Errno::last()));
    }

    Ok(())
}

/// fork() and exit from the parent, so the only running process
/// is the child process. If `wait_child` is true, rather than
/// exiting immediately, the parent will wait for the child proc
/// to exit and exit with its exit code. In the double-fork dance,
/// we want to do this the first time so we propagate the error
/// in case of a crash in the intermediate proc.
///
/// Safety: see nix::unistd::fork for preconditions.
unsafe fn become_child(wait_child: bool) -> anyhow::Result<()> {
    // Safety: since the caller has me the fork preconditions, this is
    // safe.
    match unsafe { unistd::fork() }.context("forking for daemonization")? {
        ForkResult::Parent { child } => {
            if wait_child {
                match wait::waitpid(child, None).context("waiting for child")? {
                    WaitStatus::Exited(_, status) => std::process::exit(status),
                    WaitStatus::Signaled(_, _, _) => std::process::exit(1),
                    _ => {}
                }
            } else {
                std::process::exit(0);
            }
        }
        ForkResult::Child => {}
    }
    Ok(())
}
