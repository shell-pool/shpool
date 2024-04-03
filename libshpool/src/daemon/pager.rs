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

/*!
  The pager module contains code for lauching a pager program
  such as 'less' in a background pty to display a particular
  message. This is essentially a mini version of the main shell
  loop, except that the only type of process it will launch is
  a pager, and there is no session persistence. We choose to
  re-implement a bunch of functionality instead of trying to
  re-use code because the normal shell loop has a lot of complex
  control code due to session persistence concerns and rather
  than dealing with adding even more conditions to the normal
  codepaths it seems better to have a simple and self-contained
  implementation in a dedicated module.

  This is used for displaying the motd in pager mode, though it
  works as a general out-of-band message display mechanism
  so it could potentially be used for something else in the future.
*/

use std::{
    io,
    io::{Read, Write},
    os::{
        fd::AsFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    process,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use nix::{poll, sys::signal, unistd};
use tracing::{error, info, instrument, span, trace, warn, Level};

use crate::{consts, protocol, tty};

// poll relatively quickly to pick up pager exits reasonably fast,
// but still slow enough to spend most of the time parked.
const POLL_MS: u16 = 100;

// Handles for poking at the pager from separate connection handlers.
// This is basically the same idea as for ReaderCtl.
#[derive(Debug)]
pub struct PagerCtl {
    /// Used to signal size changes so we can correctly trigger
    /// a SIGWINCH on the pty.
    pub tty_size_change: crossbeam_channel::Sender<tty::Size>,
    /// Acks the completion of a resize/SIGWINCH.
    pub tty_size_change_ack: crossbeam_channel::Receiver<()>,
}

/// Pager is capable of displaying a message to the user via
/// the pager of their choice. Display will block until they
/// quick or the connection drops.
pub struct Pager {
    /// The name of the pager program to use. Typically "less".
    pager_bin: String,
}

impl Pager {
    /// Create a new Pager.
    pub fn new(pager_bin: String) -> Self {
        Pager { pager_bin }
    }

    /// Display the message, blocking until the user quits
    /// out of the pager process or the connection drops.
    ///
    /// Though this returns any anyhow::Result, you likely want to
    /// downcast any errors to check to see if you got a PagerError,
    /// since some error conditions ought to be handled by the caller.
    /// In particular, you can't assume that the client_stream is still
    /// healthy after a call to display() and you should check for
    /// PagerError::ClientHangup to determine if you should continue
    /// on with the connection.
    ///
    /// On success, returns the final tty size, since the user may
    /// have resized their terminal while looking at the pager.
    #[instrument(skip_all)]
    pub fn display(
        &self,
        // The client connection on which to display the pager.
        client_stream: &mut UnixStream,
        // The slot to install the control handle in
        ctl_slot: Arc<Mutex<Option<PagerCtl>>>,
        // The size of the tty to start off with
        init_tty_size: tty::Size,
        // The message to display
        msg: &str,
    ) -> anyhow::Result<tty::Size> {
        let (tty_size_change_tx, tty_size_change_rx) = crossbeam_channel::bounded(0);
        let (tty_size_change_ack_tx, tty_size_change_ack_rx) = crossbeam_channel::bounded(0);
        {
            let mut ctl_handle = ctl_slot.lock().unwrap();
            if ctl_handle.is_some() {
                return Err(anyhow!("only one pager per session at a time allowed"));
            }

            trace!("registering PagerCtl");
            *ctl_handle = Some(PagerCtl {
                tty_size_change: tty_size_change_tx,
                tty_size_change_ack: tty_size_change_ack_rx,
            });
        }
        // make sure we reset the control handles
        let _ctl_guard = PagerCltGuard { ctl_slot };

        let mut msg_file = tempfile::NamedTempFile::with_prefix("shpool_pager")
            .context("creating tmp file to display msg via pager")?;
        let cleaned_msg = strip_ansi_escapes::strip(msg);
        msg_file.write_all(cleaned_msg.as_slice()).context("writing msg to tmp pager file")?;

        let mut cmd = process::Command::new(&self.pager_bin);
        cmd.arg(msg_file.path().as_os_str());

        // fork, leaving us with a handle in the master branch
        // and execing the pty wrapped pager in the child.
        info!("forking pager pty proc");
        let fork = shpool_pty::fork::Fork::from_ptmx().context("forking pty")?;
        if fork.is_child().is_ok() {
            for fd in consts::STDERR_FD + 1..(nix::unistd::SysconfVar::OPEN_MAX as i32) {
                let _ = nix::unistd::close(fd);
            }
            let err = cmd.exec();
            eprintln!("pager exec err: {:?}", err);
            std::process::exit(1);
        }
        let pager_exited = Arc::new(AtomicBool::new(false));
        let _proc_guard =
            PagerProcGuard { pager_proc: &fork, pager_exited: Arc::clone(&pager_exited) };

        let pager_exited_ref = Arc::clone(&pager_exited);
        let waitable_child = fork.clone();
        thread::spawn(move || {
            let _s = span!(Level::INFO, "pager_exit_monitor").entered();
            match waitable_child.wait_for_exit() {
                Ok((_, Some(exit_status))) => {
                    info!("child pager exited with status {}", exit_status);
                    pager_exited_ref.store(true, Ordering::Relaxed);
                }
                Ok((_, None)) => {
                    info!("child pager exited without status");
                    pager_exited_ref.store(true, Ordering::Relaxed);
                }
                Err(e) => {
                    info!("error waiting on pager child: {:?}", e);
                    pager_exited_ref.store(true, Ordering::Relaxed);
                }
            }
            info!("reaped child pager: {:?}", waitable_child);
        });

        let mut pty_master = fork.is_parent().context("getting pty_master handle")?;

        // spawn a background thread to handle tty size change events,
        // setting it up to go away when _ctl_guard removes the ctl
        // handle.
        let pty_master_fd = pty_master.raw_fd().ok_or(anyhow!("no fd for pty master"))?;
        init_tty_size.set_fd(pty_master_fd).context("setting init tty size")?;
        let tty_size = Arc::new(Mutex::new(init_tty_size.clone()));
        let tty_size_ref = Arc::clone(&tty_size);
        info!("spawning pager size change listener");
        thread::spawn(move || {
            let _s = span!(Level::INFO, "pager_size_change").entered();

            // We could also set things up to handle detach commands, but
            // since pagers don't stick around when the client hangs up
            // it is not really that importaint. Let's KISS.
            while let Ok(size) = tty_size_change_rx.recv() {
                if let Err(e) = size.set_fd(pty_master_fd) {
                    warn!("setting pager size: {:?}", e);
                }

                {
                    // register the new size so it will get returned
                    let mut tty_size = tty_size_ref.lock().unwrap();
                    *tty_size = size;
                }

                if let Err(e) = tty_size_change_ack_tx.send(()) {
                    error!("could not send size change ack: {:?}", e);
                    break;
                }
            }
            info!("pager size change loop done");
        });

        let mut last_heartbeat_at = Instant::now();
        let mut buf = vec![0; consts::BUF_SIZE];
        let watchable_master = pty_master;
        let watchable_client_stream =
            client_stream.try_clone().context("could not clone client stream")?;
        loop {
            // wake up when there is data for us going in either direction
            let mut poll_fds = [
                poll::PollFd::new(
                    watchable_master.borrow_fd().ok_or(anyhow!("no master fd"))?,
                    poll::PollFlags::POLLIN,
                ),
                poll::PollFd::new(watchable_client_stream.as_fd(), poll::PollFlags::POLLIN),
            ];
            let nready = poll::poll(&mut poll_fds, POLL_MS).context("polling both streams")?;
            if pager_exited.load(Ordering::Relaxed) {
                let tty_size = tty_size.lock().unwrap();
                return Ok(tty_size.clone());
            }
            if nready == 0 {
                // if timeout
                let now = Instant::now();
                if now
                    .checked_duration_since(
                        last_heartbeat_at
                            .checked_add(consts::HEARTBEAT_DURATION)
                            .ok_or(anyhow!("could not add to dur"))?,
                    )
                    .is_some()
                {
                    last_heartbeat_at = now;

                    let chunk = protocol::Chunk { kind: protocol::ChunkKind::Heartbeat, buf: &[] };
                    match chunk.write_to(client_stream).and_then(|_| client_stream.flush()) {
                        Ok(_) => {
                            trace!("wrote heartbeat");
                        }
                        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                            trace!("client hangup writing heartbeat: {:?}", e);
                            return Err(PagerError::ClientHangup)?;
                        }
                        Err(e) => {
                            return Err(e).context("writing heartbeat")?;
                        }
                    }
                }
            } else {
                // -1 case should have been turned into an error already
                assert!(nready > 0);
                let pty_master_poll_fd = &poll_fds[0];
                let client_stream_poll_fd = &poll_fds[1];

                if pty_master_poll_fd.any().unwrap_or(false) {
                    // the pager process has some data for us
                    let len = pty_master.read(&mut buf).context("reading chunk from pty master")?;
                    let chunk =
                        protocol::Chunk { kind: protocol::ChunkKind::Data, buf: &buf[..len] };
                    match chunk.write_to(client_stream).and_then(|_| client_stream.flush()) {
                        Ok(_) => {}
                        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                            trace!("client hangup writing data chunk: {:?}", e);
                            return Err(PagerError::ClientHangup)?;
                        }
                        Err(e) => {
                            return Err(e).context("writing data chunk")?;
                        }
                    }
                }

                if client_stream_poll_fd.any().unwrap_or(false) {
                    let len = client_stream.read(&mut buf).context("reading client chunk")?;
                    if len == 0 {
                        continue;
                    }

                    trace!("user input: {}", String::from_utf8_lossy(&buf[..len]));

                    if let Err(e) = pty_master.write_all(&buf[0..len]) {
                        info!("Error writing to pager pty, nbd though: {:?}", e);
                        // assume the pager proc just quit normally and the
                        // timing was such that we didn't pick it up with our
                        // exit watcher thread.
                        let tty_size = tty_size.lock().unwrap();
                        return Ok(tty_size.clone());
                    }
                    if let Err(e) = pty_master.flush() {
                        info!("Error flushing pager pty, nbd though: {:?}", e);
                        // same logic as above
                        let tty_size = tty_size.lock().unwrap();
                        return Ok(tty_size.clone());
                    }
                }
            }
        }
    }
}

// An RAII guard to make sure that we reset the pager_ctl
// slot in the session struct.
struct PagerCltGuard {
    ctl_slot: Arc<Mutex<Option<PagerCtl>>>,
}

impl std::ops::Drop for PagerCltGuard {
    fn drop(&mut self) {
        let mut pager_ctl = self.ctl_slot.lock().unwrap();
        // N.B. clobbering the handles here will cause the listening
        // thread to exit because it drops the senders. This ensures
        // that no callers can grab the lock on the ctl handles and
        // then make a call when no one is listening.
        *pager_ctl = None;
        trace!("deregistered PagerCtl");
    }
}

/// An RAII guard to make sure that the pager process is for
/// sure gone by the time the display routine exits.
struct PagerProcGuard<'pager> {
    pager_proc: &'pager shpool_pty::fork::Fork,
    /// Used to make sure we don't try to kill the child proc
    /// if it has already exited on its own.
    pager_exited: Arc<AtomicBool>,
}

impl<'pager> std::ops::Drop for PagerProcGuard<'pager> {
    fn drop(&mut self) {
        if self.pager_exited.load(Ordering::Relaxed) {
            // our work here is done
            return;
        }

        if let Err(e) = self.kill() {
            error!("Error cleaning up pager proc: {:?}", e);
        }
    }
}

impl<'pager> PagerProcGuard<'pager> {
    fn kill(&self) -> anyhow::Result<()> {
        let pid = if let shpool_pty::fork::Fork::Parent(pid, _) = self.pager_proc {
            *pid
        } else {
            return Err(anyhow!("somehow have a child pty handle in the main proc"));
        };

        // first we'll be polite and give a SIGTERM
        signal::kill(unistd::Pid::from_raw(pid), Some(signal::Signal::SIGTERM))
            .context("sending SIGTERM to pager proc")?;

        // now do an exponential backoff for
        // `sum(10*(2**x) for x in range(7))` = 1270 ms = ~1.2 s
        // to see if that worked
        let mut sleep_ms = 10;
        for _ in 0..7 {
            if self.pager_exited.load(Ordering::Relaxed) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms *= 2;
        }

        // now we are done asking
        signal::kill(unistd::Pid::from_raw(pid), Some(signal::Signal::SIGKILL))
            .context("sending SIGKILL to pager proc")?;

        // and we won't stick around to see if that worked because we
        // have no further way to escalate

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum PagerError {
    /// Indicates that the client stream was closed by the client
    /// while we were showing them the pager.
    ClientHangup,
}

impl std::fmt::Display for PagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

impl std::error::Error for PagerError {}
