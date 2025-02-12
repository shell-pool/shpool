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
    io,
    io::{Read, Write},
    net,
    ops::Add,
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread, time,
    time::Duration,
};

use anyhow::{anyhow, Context};
use nix::{sys::signal, unistd::Pid};
use shpool_protocol::{Chunk, ChunkKind, TtySize};
use tracing::{debug, error, info, instrument, span, trace, warn, Level};

use crate::{
    consts,
    daemon::{config, exit_notify::ExitNotifier, keybindings, pager::PagerCtl, prompt, show_motd},
    protocol::ChunkExt as _,
    test_hooks,
    tty::TtySizeExt as _,
};

// To prevent data getting dropped, we set this to be large, but we don't want
// to use u16::MAX, since the vt100 crate eagerly fills in its rows, and doing
// so is very memory intensive. The right fix is to get the vt100 crate to
// lazily initialize its rows, but that is likely a bunch of work.
const VTERM_WIDTH: u16 = 1024;

const SHELL_KILL_TIMEOUT: time::Duration = time::Duration::from_millis(500);

const SUPERVISOR_POLL_DUR: time::Duration = time::Duration::from_millis(300);

// Chosen experimentally. This value is small enough that no human will likely
// recognize it, and it seems to be large enough that emacs consistently picks
// up the "jiggle" trick where we oversize the pty then put it back to the right
// size.
const REATTACH_RESIZE_DELAY: time::Duration = time::Duration::from_millis(50);

// The shell->client thread should wake up relatively frequently so it can
// detect reattach, but we don't need to go crazy since reattach is not part of
// the inner loop.
const SHELL_TO_CLIENT_POLL_MS: u16 = 100;

// How long to wait before giving up while trying to talk to the
// shell->client thread.
const SHELL_TO_CLIENT_CTL_TIMEOUT: time::Duration = time::Duration::from_millis(300);

/// Session represent a shell session
#[derive(Debug)]
pub struct Session {
    pub started_at: time::SystemTime,
    pub child_pid: libc::pid_t,
    pub child_exit_notifier: Arc<ExitNotifier>,
    pub shell_to_client_ctl: Arc<Mutex<ReaderCtl>>,
    pub pager_ctl: Arc<Mutex<Option<PagerCtl>>>,
    /// Mutable state with the lock held by the servicing handle_attach thread
    /// while a tty is attached to the session. Probing the mutex can be used
    /// to determine if someone is currently attached to the session.
    pub inner: Arc<Mutex<SessionInner>>,
}

impl Session {
    /// Kill the session, first sending a SIGHUP and then resorting to a
    /// SIGKILL if that doesn't work (SIGTERM doesn't really work on shells).
    #[instrument(skip_all)]
    pub fn kill(&self) -> anyhow::Result<()> {
        // SIGHUP is a signal to indicate that the terminal has disconnected
        // from a process. We can't use the normal SIGTERM graceful-shutdown
        // signal since shells just forward those to their child process,
        // but for shells SIGHUP serves as the graceful shutdown signal.
        signal::kill(Pid::from_raw(self.child_pid), Some(signal::Signal::SIGHUP))
            .context("sending SIGHUP to child proc")?;

        if self.child_exit_notifier.wait(Some(SHELL_KILL_TIMEOUT)).is_none() {
            info!("child failed to exit within kill timeout, no longer being polite");
            signal::kill(Pid::from_raw(self.child_pid), Some(signal::Signal::SIGKILL))
                .context("sending SIGKILL to child proc")?;
        }

        Ok(())
    }
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
pub struct SessionInner {
    pub name: String, // to improve logging
    pub shell_to_client_ctl: Arc<Mutex<ReaderCtl>>,
    pub pty_master: shpool_pty::fork::Fork,
    pub client_stream: Option<UnixStream>,
    pub config: config::Manager,
    pub term_db: Arc<termini::TermInfo>,
    pub daily_messenger: Arc<show_motd::DailyMessenger>,
    pub needs_initial_motd_dump: bool,
    pub custom_cmd: bool,

    /// The join handle for the always-on background shell->client thread.
    /// Only wrapped in an option so we can spawn the thread after
    /// constructing the SessionInner.
    pub shell_to_client_join_h: Option<thread::JoinHandle<anyhow::Result<()>>>,
}

/// A notification that a new client has connected, sent to the
/// shell->client thread.
pub struct ClientConnection {
    /// All output data should be written to this sink rather than
    /// directly to the unix stream.
    sink: io::BufWriter<UnixStream>,
    /// The size of the client tty.
    size: TtySize,
    /// The raw unix socket stream. The shell->client thread should
    /// never write to this directly, just use it for control
    /// operations like shutdown.
    stream: UnixStream,
}

#[derive(Debug)]
pub enum ClientConnectionStatus {
    /// The new session replaced an existing session client.
    Replaced,
    /// The new session attached to a shell with no existing client.
    New,
    /// We detached an existing client.
    Detached,
    /// An instruction to detach had no effect, since there was already
    /// no client attached.
    DetachNone,
}

struct ResizeCmd {
    /// The actual size to set to
    size: TtySize,
    /// Only perform the resize after this point in time.
    /// Allows for delays to work around emacs being a special
    /// snowflake.
    when: time::Instant,
}

fn log_if_error<T, E>(ctx: &str, res: Result<T, E>) -> Result<T, E>
where
    E: std::fmt::Debug,
{
    res.map_err(|e| {
        error!("{}: {:?}", ctx, e);
        e
    })
}

/// Messages to the shell->client thread to add or remove a client connection.
pub enum ClientConnectionMsg {
    /// Accept a newly connected client
    New(ClientConnection),
    /// Disconnect the client and exit the shell->client loop since
    /// the client shell has exited with the given exit status.
    DisconnectExit(i32),
    /// Disconnect the client, but stay around and be ready for
    /// reconnects.
    Disconnect,
}

pub struct ReaderArgs {
    pub conn_id: usize,
    pub tty_size: TtySize,
    pub scrollback_lines: usize,
    pub session_restore_mode: config::SessionRestoreMode,
    pub client_connection: crossbeam_channel::Receiver<ClientConnectionMsg>,
    pub client_connection_ack: crossbeam_channel::Sender<ClientConnectionStatus>,
    pub tty_size_change: crossbeam_channel::Receiver<TtySize>,
    pub tty_size_change_ack: crossbeam_channel::Sender<()>,
    pub heartbeat: crossbeam_channel::Receiver<()>,
    // true if the client is still live, false if it has hung up on us
    pub heartbeat_ack: crossbeam_channel::Sender<bool>,
}

impl SessionInner {
    /// Spawn the shell-to-client thread which continually reads from the pty
    /// and sends data both to the output spool and to the client,
    /// if one is attached.
    #[instrument(skip_all, fields(s = self.name))]
    pub fn spawn_shell_to_client(
        &self,
        args: ReaderArgs,
    ) -> anyhow::Result<thread::JoinHandle<anyhow::Result<()>>> {
        use nix::poll;

        let term_db = Arc::clone(&self.term_db);
        let mut prompt_sentinel_scanner = prompt::SentinelScanner::new(consts::PROMPT_SENTINEL);

        // We only scan for the prompt sentinel if the user has not set up a
        // custom command or blanked out the prompt_prefix config option.
        let prompt_prefix_is_blank =
            self.config.get().prompt_prefix.as_ref().map(|p| p.is_empty()).unwrap_or(false);
        let mut has_seen_prompt_sentinel = self.custom_cmd || prompt_prefix_is_blank;

        let daily_messenger = Arc::clone(&self.daily_messenger);
        let mut needs_initial_motd_dump = self.needs_initial_motd_dump;

        let vterm_width = {
            let config = self.config.get();
            config.vt100_output_spool_width.unwrap_or(VTERM_WIDTH)
        };
        let mut pty_master = self.pty_master.is_parent()?;
        let watchable_master = pty_master;
        let name = self.name.clone();
        let mut closure = move || {
            let _s = span!(Level::INFO, "shell->client", s = name, cid = args.conn_id).entered();

            let mut output_spool =
                if matches!(args.session_restore_mode, config::SessionRestoreMode::Simple) {
                    None
                } else {
                    Some(shpool_vt100::Parser::new(
                        args.tty_size.rows,
                        vterm_width,
                        args.scrollback_lines,
                    ))
                };
            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];
            let mut poll_fds = [poll::PollFd::new(
                watchable_master.borrow_fd().ok_or(anyhow!("no master fd"))?,
                poll::PollFlags::POLLIN,
            )];

            // block until we get the first connection attached so that we don't drop
            // the initial prompt on the floor
            info!("waiting for initial client connection");
            let mut client_conn: ClientConnectionMsg =
                args.client_connection.recv().context("waiting for initial client connection")?;
            args.client_connection_ack
                .send(ClientConnectionStatus::New)
                .context("sending initial client connection ack")?;
            info!("got initial client connection");

            let mut resize_cmd = if let ClientConnectionMsg::New(conn) = &client_conn {
                Some(ResizeCmd { size: conn.size.clone(), when: time::Instant::now() })
            } else {
                None
            };

            loop {
                let mut do_reattach = false;
                crossbeam_channel::select! {
                    recv(args.client_connection) -> new_connection => {
                        match new_connection {
                            Ok(ClientConnectionMsg::New(conn)) => {
                                let _s = span!(Level::INFO, "HANDLE_NEW_CONN");
                                info!("got new connection (rows={}, cols={})", conn.size.rows, conn.size.cols);
                                info!("1");
                                do_reattach = true;
                                info!("2");
                                let ack = if let ClientConnectionMsg::New(mut old_conn) = client_conn {
                                    info!("3");
                                    Self::write_exit_chunk(&mut old_conn.sink, 0);
                                    info!("4");
                                    old_conn.stream.shutdown(net::Shutdown::Both)?;
                                    info!("5");
                                    ClientConnectionStatus::Replaced
                                } else {
                                    info!("6");
                                    ClientConnectionStatus::New
                                };
                                info!("7");
                                // Resize the pty to be bigger than it needs to be,
                                // we do this immediately so that the extra size
                                // can "bake" for a little bit, which emacs seems
                                // to require in order to pick up the jiggle.
                                let oversize = TtySize {
                                    rows: conn.size.rows + 1,
                                    cols: conn.size.cols + 1,
                                    xpixel: conn.size.xpixel,
                                    ypixel: conn.size.ypixel,
                                };
                                info!("8");
                                oversize.set_fd(pty_master.raw_fd().ok_or(anyhow!("no master fd"))?)?;
                                info!("9");

                                // Always instantly resize the spool, since we don't
                                // need to inject a delay into that.
                                if let Some(s) = output_spool.as_mut() {
                                    info!("10");
                                    s.screen_mut().set_size(conn.size.rows, VTERM_WIDTH);
                                }
                                info!("11");
                                resize_cmd = Some(ResizeCmd {
                                    size: conn.size.clone(),
                                    when: time::Instant::now().add(REATTACH_RESIZE_DELAY),
                                });
                                info!("12");
                                client_conn = ClientConnectionMsg::New(conn);
                                info!("13");

                                args.client_connection_ack.send(ack)
                                    .context("sending client connection ack")?;
                            }
                            Ok(ClientConnectionMsg::Disconnect) => {
                                let ack = if let ClientConnectionMsg::New(mut old_conn) = client_conn {
                                    info!("disconnect, shutting down client stream");
                                    Self::write_exit_chunk(&mut old_conn.sink, 0);
                                    old_conn.stream.shutdown(net::Shutdown::Both)?;
                                    ClientConnectionStatus::Detached
                                } else {
                                    info!("disconnect, no client stream to shut down");
                                    ClientConnectionStatus::DetachNone
                                };
                                client_conn = ClientConnectionMsg::Disconnect;

                                args.client_connection_ack.send(ack)
                                    .context("sending client disconnect ack")?;
                            }
                            Ok(ClientConnectionMsg::DisconnectExit(exit_status)) => {
                                let ack = if let ClientConnectionMsg::New(mut old_conn) = client_conn {
                                    info!("disconnectexit({}), shutting down client stream",
                                           exit_status);

                                    // write an exit status frame so the attach process
                                    // can exit with the same exit code as the child shell
                                    Self::write_exit_chunk(&mut old_conn.sink, exit_status);
                                    old_conn.stream.shutdown(net::Shutdown::Both)?;

                                    ClientConnectionStatus::Detached
                                } else {
                                    info!(
                                        "disconnectexit({}), no client stream to shut down",
                                          exit_status);
                                    ClientConnectionStatus::DetachNone
                                };
                                args.client_connection_ack.send(ack)
                                    .context("sending client disconnect exit ack")?;

                                return Ok(());
                            }

                            // SessionInner getting dropped, so this thread should go away.
                            Err(crossbeam_channel::RecvError) => {
                                info!("client conn: bailing due to RecvError");
                                return Ok(())
                            },
                        }
                    }
                    recv(args.tty_size_change) -> new_size => {
                        match new_size {
                            Ok(size) => {
                                info!("resize size={:?}", size);
                                if let Some(s) = output_spool.as_mut() {
                                    s.screen_mut().set_size(size.rows, VTERM_WIDTH);
                                }
                                resize_cmd = Some(ResizeCmd {
                                    size,
                                    // No delay needed for ordinary resizes, just
                                    // for reconnects.
                                    when: time::Instant::now(),
                                });
                                args.tty_size_change_ack.send(())
                                    .context("sending size change ack")?;
                            }
                            Err(err) => {
                                warn!("size change: bailing due to: {:?}", err);
                                return Ok(());
                            }
                        }
                    }
                    recv(args.heartbeat) -> _ => {
                        let client_present = if let ClientConnectionMsg::New(conn) = &mut client_conn {
                            let chunk = Chunk { kind: ChunkKind::Heartbeat, buf: &[] };
                            match chunk.write_to(&mut conn.sink).and_then(|_| conn.sink.flush()) {
                                Ok(_) => {
                                    trace!("wrote heartbeat");
                                    true
                                }
                                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                                    trace!("client hangup: {:?}", e);
                                    false
                                }
                                Err(e) => {
                                    error!("unexpected IO error while writing heartbeat: {}", e);
                                    return Err(e).context("writing heartbeat")?;
                                }
                            }
                        } else {
                            false
                        };

                        args.heartbeat_ack.send(client_present)
                            .context("sending heartbeat ack")?;
                    }

                    // make this select non-blocking so we spend most of our time parked
                    // in poll
                    default => {}
                }

                let mut executed_resize = false;
                if let Some(resize_cmd) = resize_cmd.as_ref() {
                    if resize_cmd.when.saturating_duration_since(time::Instant::now())
                        == time::Duration::ZERO
                    {
                        let status = pty_master
                            .raw_fd()
                            .ok_or(anyhow!("no master fd"))
                            .and_then(|fd| resize_cmd.size.set_fd(fd));
                        if let Err(e) = status {
                            warn!("error resizing pty: {}", e);
                        }
                        executed_resize = true;
                        info!(
                            "resized fd (rows={}, cols={})",
                            resize_cmd.size.rows, resize_cmd.size.cols
                        );
                    }
                }
                if executed_resize {
                    resize_cmd = None;
                }

                if do_reattach {
                    use config::SessionRestoreMode::*;

                    info!("executing reattach protocol (mode={:?})", args.session_restore_mode);
                    let restore_buf = match (output_spool.as_mut(), &args.session_restore_mode) {
                        (Some(spool), Screen) => {
                            let (rows, cols) = spool.screen().size();
                            info!(
                                "computing screen restore buf with (rows={}, cols={})",
                                rows, cols
                            );
                            spool.screen().contents_formatted()
                        }
                        (Some(spool), Lines(nlines)) => {
                            let (_, cols) = spool.screen().size();
                            info!("computing lines({}) restore buf with (cols={})", nlines, cols);
                            spool.screen().last_n_rows_contents_formatted(*nlines)
                        }
                        (_, _) => vec![],
                    };
                    if let (true, ClientConnectionMsg::New(conn)) =
                        (!restore_buf.is_empty(), &mut client_conn)
                    {
                        trace!("restore chunk='{}'", String::from_utf8_lossy(&restore_buf[..]));
                        // send the restore buffer, broken up into chunks so that we don't make
                        // the client allocate too much
                        for block in restore_buf.as_slice().chunks(consts::BUF_SIZE) {
                            let chunk = Chunk { kind: ChunkKind::Data, buf: block };

                            if let Err(err) = chunk.write_to(&mut conn.sink) {
                                warn!("err writing session-restore buf: {:?}", err);
                            }
                        }
                        if let Err(err) = conn.sink.flush() {
                            warn!("err flushing session-restore: {:?}", err);
                        }
                    }
                }

                // TODO(ethan): what if poll times out on a tick when we have just
                // set up a restore chunk? It looks like we will just drop the
                // data as things are now.

                // Block until the shell has some data for us so we can be sure our reads
                // always succeed. We don't want to end up blocked forever on a read while
                // a client is trying to attach.
                let nready = match poll::poll(&mut poll_fds, SHELL_TO_CLIENT_POLL_MS) {
                    Ok(n) => n,
                    Err(e) => {
                        error!("polling pty master: {:?}", e);
                        return Err(e)?;
                    }
                };
                if nready == 0 {
                    // if timeout
                    continue;
                }
                if nready != 1 {
                    return Err(anyhow!("shell->client thread: expected exactly 1 ready fd"));
                }
                let len = match pty_master.read(&mut buf) {
                    Ok(l) => l,
                    Err(e) => {
                        error!("reading chunk from pty master: {:?}", e);
                        return Err(e).context("reading pty master chunk")?;
                    }
                };
                if len == 0 {
                    continue;
                }
                let mut buf = &buf[..len];
                trace!("read pty master len={} '{}'", len, String::from_utf8_lossy(buf));

                // scan for control codes we need to handle
                if !has_seen_prompt_sentinel {
                    for (i, byte) in buf.iter().enumerate() {
                        if prompt_sentinel_scanner.transition(*byte) {
                            info!("saw prompt sentinel");
                            // This will cause us to start actually sending data frames back to
                            // the client.
                            has_seen_prompt_sentinel = true;

                            // drop everything up to and including the sentinel
                            buf = &buf[i + 1..];
                        }
                    }
                }

                if !matches!(args.session_restore_mode, config::SessionRestoreMode::Simple) {
                    if let (Some(s), true) = (output_spool.as_mut(), has_seen_prompt_sentinel) {
                        s.process(buf);
                    }
                }

                let mut reset_client_conn = false;
                if let (ClientConnectionMsg::New(conn), true) =
                    (&mut client_conn, has_seen_prompt_sentinel)
                {
                    let chunk = Chunk { kind: ChunkKind::Data, buf };

                    // If we still need to do an initial motd dump, it means we have just finished
                    // dropping all the prompt setup stuff, we should dump the motd now before we
                    // write the first chunk.
                    if needs_initial_motd_dump {
                        needs_initial_motd_dump = false;
                        if let Err(e) = daily_messenger.dump(&mut conn.sink, &term_db) {
                            warn!("Error handling clear: {:?}", e);
                        }
                    }

                    let write_result =
                        chunk.write_to(&mut conn.sink).and_then(|_| conn.sink.flush());
                    if let Err(err) = write_result {
                        info!("client_stream write err, assuming hangup: {:?}", err);
                        reset_client_conn = true;
                    } else {
                        test_hooks::emit("daemon-wrote-s2c-chunk");
                    }
                }
                if reset_client_conn {
                    client_conn = ClientConnectionMsg::Disconnect;
                }
            }
        };

        Ok(thread::Builder::new()
            .name(format!("shell->client({})", self.name))
            .spawn(move || log_if_error("error in shell->client", closure()))?)
    }

    fn write_exit_chunk<W: io::Write>(mut sink: W, status: i32) {
        let status_buf: [u8; 4] = status.to_le_bytes();
        let chunk = Chunk { kind: ChunkKind::ExitStatus, buf: status_buf.as_slice() };
        match chunk.write_to(&mut sink).and_then(|_| sink.flush()) {
            Ok(_) => {
                trace!("wrote exit status chunk");
            }
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                trace!("client hangup: {:?}", e);
            }
            Err(e) => {
                error!("writing exit status chunk: {:?}", e);
            }
        };
    }

    /// bidi_stream shuffles bytes between the subprocess and
    /// the client connection. It returns true if the subprocess
    /// has exited, and false if it is still running.
    #[instrument(skip_all, fields(s = self.name))]
    pub fn bidi_stream(
        &mut self,
        conn_id: usize,
        init_tty_size: TtySize,
        child_exit_notifier: Arc<ExitNotifier>,
    ) -> anyhow::Result<bool> {
        test_hooks::emit("daemon-bidi-stream-enter");
        #[allow(clippy::let_unit_value)]
        let _bidi_stream_test_guard = test_hooks::scoped("daemon-bidi-stream-done");

        // we take the client stream so that it gets closed when this routine
        // returns
        let client_stream = match self.client_stream.take() {
            Some(s) => s,
            None => return Err(anyhow!("no client stream to take for bidi streaming")),
        };

        let mut client_to_shell_client_stream =
            client_stream.try_clone().context("creating client->shell client stream")?;
        let shell_to_client_client_stream =
            client_stream.try_clone().context("creating shell->client client stream handle")?;
        let output_sink =
            io::BufWriter::new(client_stream.try_clone().context("wrapping stream in bufwriter")?);

        {
            let _s = span!(Level::INFO, "initial_attach_lock(shell_to_client_ctl)").entered();
            let shell_to_client_ctl = self.shell_to_client_ctl.lock().unwrap();
            shell_to_client_ctl
                .client_connection
                .send_timeout(
                    ClientConnectionMsg::New(ClientConnection {
                        sink: output_sink,
                        size: init_tty_size,
                        stream: shell_to_client_client_stream,
                    }),
                    SHELL_TO_CLIENT_CTL_TIMEOUT,
                )
                .context("attaching new client stream to shell->client thread")?;
            let status = shell_to_client_ctl
                .client_connection_ack
                .recv_timeout(SHELL_TO_CLIENT_CTL_TIMEOUT)
                .context("waiting for client connection ack (1)")?;
            info!("client connection status={:?}", status);
        }

        let pty_master =
            self.pty_master.is_parent().context("internal error: executing in child fork")?;

        // A flag to indicate that outstanding threads should stop
        let stop = AtomicBool::new(false);
        // A flag to indicate if the child shell has exited
        let child_done = AtomicBool::new(false);

        thread::scope(|s| -> anyhow::Result<()> {
            // Spawn the main data transport threads
            let client_to_shell_h = self.spawn_client_to_shell(
                s, conn_id, &stop, &pty_master, &mut client_to_shell_client_stream)?;

            // Send a steady stream of heartbeats to the client
            // so that if the connection unexpectedly goes
            // down, we detect it immediately.
            let heartbeat_h = self.spawn_heartbeat(s, conn_id, &stop)?;

            // poll the pty master fd to see if the child
            // shell has exited.
            let supervisor_h = self.spawn_supervisor(
                s, conn_id, &stop, &child_done, &pty_master,
                Arc::clone(&child_exit_notifier))?;

            loop {
                let c_done = child_done.load(Ordering::Acquire);
                if client_to_shell_h.is_finished()
                    || heartbeat_h.is_finished() || supervisor_h.is_finished() || c_done {
                    debug!("signaling for threads to stop: client_to_shell_finished={} heartbeat_finished={} supervisor_finished={} child_done={}",
                        client_to_shell_h.is_finished(),
                        heartbeat_h.is_finished(),
                        supervisor_h.is_finished(),
                        c_done,
                    );
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                thread::sleep(consts::JOIN_POLL_DURATION);
            }

            // disconnect the shell->client thread first since that will shutdown the
            // client stream, and the client->shell thread hangs out blocked on
            // that stream, so we need to close it in order to get all our
            // cows to come home.
            let c_done = child_done.load(Ordering::Acquire);
            {
                let _s = span!(Level::INFO, "disconnect_lock(shell_to_client_ctl)").entered();
                let shell_to_client_ctl = self.shell_to_client_ctl.lock().unwrap();
                let send_res = shell_to_client_ctl.client_connection.send_timeout(if c_done {
                    let exit_status = child_exit_notifier
                        .wait(Some(Duration::from_secs(0)))
                        .unwrap_or(1);
                    info!("telling shell->client to disconnect with exit status {}", exit_status);
                    ClientConnectionMsg::DisconnectExit(exit_status)
                } else {
                    info!("telling shell->client to disconnect without reaping");
                    ClientConnectionMsg::Disconnect
                }, SHELL_TO_CLIENT_CTL_TIMEOUT);

                if let Err(send_timeout_err) = send_res {
                    info!("failed to tell shell->client to disconnect: {:?}", send_timeout_err);

                    // the shell->client didn't close the client stream for us, so we'll need
                    // to handle that ourselves
                    client_stream.shutdown(net::Shutdown::Both)?;
                } else {
                    let status = shell_to_client_ctl.client_connection_ack.recv_timeout(SHELL_TO_CLIENT_CTL_TIMEOUT)
                        .context("waiting for client connection ack (2)")?;
                    info!("detached from shell->client, status = {:?}", status);
                }
            }

            debug!("joining client_to_shell_h");
            match client_to_shell_h.join() {
                Ok(v) => v.context("joining client_to_shell_h")?,
                Err(panic_err) => {
                    debug!("client_to_shell panic_err = {:?}", panic_err);
                    std::panic::resume_unwind(panic_err)
                },
            }
            debug!("joining heartbeat_h");
            match heartbeat_h.join() {
                Ok(v) => v.context("joining heartbeat_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            debug!("joining supervisor_h");
            match supervisor_h.join() {
                Ok(v) => v.context("joining supervisor_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            debug!("joined all threads");

            Ok(())
        }).context("outer thread scope")?;

        let c_done = child_done.load(Ordering::Acquire);
        if c_done {
            client_stream
                .shutdown(std::net::Shutdown::Both)
                .context("shutting down client stream")?;
        }

        info!("done child_done={}", c_done);
        Ok(c_done)
    }

    #[instrument(skip_all)]
    fn spawn_client_to_shell<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        conn_id: usize,
        stop: &'scope AtomicBool,
        pty_master: &'scope shpool_pty::fork::Master,
        shell_to_client_client_stream: &'scope mut UnixStream,
    ) -> anyhow::Result<thread::ScopedJoinHandle<'scope, anyhow::Result<()>>> {
        let empty_bindings = vec![config::Keybinding {
            binding: String::from("Ctrl-Space Ctrl-q"),
            action: keybindings::Action::Detach,
        }];
        let bindings = keybindings::Bindings::new(
            self.config
                .get()
                .keybinding
                .as_ref()
                .unwrap_or(&empty_bindings)
                .iter()
                .map(|binding| (binding.binding.as_str(), binding.action)),
        );

        thread::Builder::new()
            .name(format!("client->shell({})", self.name))
            .spawn_scoped(scope, move || -> anyhow::Result<()> {
                let _s =
                    span!(Level::INFO, "client->shell", s = self.name, cid = conn_id).entered();
                let mut bindings = bindings.context("compiling keybindings engine")?;

                let mut master_writer = *pty_master;

                let mut snip_sections = vec![]; // (<len>, <end offset>)
                let mut keep_sections = vec![]; // (<start offset>, <end offset>)
                let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];
                let mut partial_keybinding = vec![];

                loop {
                    if stop.load(Ordering::Relaxed) {
                        info!("recvd stop msg (1)");
                        return Ok(());
                    }

                    // N.B. we don't need to muck about with chunking or anything
                    // in this direction, because there is only one input stream
                    // to the shell subprocess and we don't need to worry about
                    // heartbeating to detect hangup or anything like that.
                    //
                    // Also, note that we don't access through the mutex because reads
                    // don't need to be excluded from trampling on writes.
                    let mut len = shell_to_client_client_stream
                        .read(&mut buf)
                        .context("reading client chunk")?;
                    if len == 0 {
                        continue;
                    }
                    test_hooks::emit("daemon-read-c2s-chunk");
                    trace!("read client len={}: '{}'", len, String::from_utf8_lossy(&buf[..len]),);

                    // We might be able to gain some perf by doing this scanning in
                    // a background thread (though maybe not given the need to copy
                    // the data), but just doing it inline doesn't seem have have
                    // a major perf impact, and this way is simpler.
                    snip_sections.clear();
                    for (i, byte) in buf[0..len].iter().enumerate() {
                        use keybindings::BindingResult::*;
                        match bindings.transition(*byte) {
                            NoMatch
                                if !partial_keybinding.is_empty()
                                    && i < partial_keybinding.len() =>
                            {
                                // it turned out the partial keybinding match was not
                                // a real match, so flush it to the output stream
                                debug!(
                                    "flushing partial keybinding_len={} i={}",
                                    partial_keybinding.len(),
                                    i
                                );
                                master_writer
                                    .write_all(&partial_keybinding)
                                    .context("writing partial keybinding")?;
                                if i > 0 {
                                    // snip the leading part of the input chunk that
                                    // was part of this keybinding
                                    snip_sections.push((i, i - 1));
                                }
                                partial_keybinding.clear()
                            }
                            NoMatch => {
                                partial_keybinding.clear();
                            }
                            Partial => {
                                partial_keybinding.push(*byte);
                            }
                            Match(action) => {
                                info!("{:?} keybinding action fired", action);
                                let keybinding_len = partial_keybinding.len() + 1;
                                if keybinding_len < i {
                                    // this keybinding is wholly contained in buf
                                    debug!("snipping keybinding_len={} i={}", keybinding_len, i);
                                    snip_sections.push((keybinding_len, i));
                                } else {
                                    // this keybinding was split across multiple
                                    // input buffers, just snip the last bit
                                    debug!("snipping split keybinding i={}", i);
                                    snip_sections.push((i + 1, i));
                                }
                                partial_keybinding.clear();

                                use keybindings::Action::*;
                                match action {
                                    Detach => self.action_detach()?,
                                    NoOp => {}
                                }
                            }
                        }
                    }
                    if !partial_keybinding.is_empty() {
                        // we have a partial keybinding pending, so don't write
                        // it to the output stream immediately
                        let snip_chunk_len = if partial_keybinding.len() > len {
                            len
                        } else {
                            partial_keybinding.len()
                        };
                        debug!(
                            "end of buf w/ partial keybinding_len={} snip_chunk_len={} buf_len={}",
                            partial_keybinding.len(),
                            snip_chunk_len,
                            len
                        );
                        snip_sections.push((snip_chunk_len, len - 1));
                    }
                    len = snip_buf(&mut buf[..], len, &snip_sections[..], &mut keep_sections);

                    master_writer.write_all(&buf[0..len]).context("writing client chunk")?;

                    master_writer.flush().context("flushing input from client to shell")?;

                    debug!("flushed chunk of len {}", len);
                }
            })
            .map_err(|e| anyhow!("{:?}", e))
    }

    #[instrument(skip_all)]
    fn spawn_heartbeat<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        conn_id: usize,
        stop: &'scope AtomicBool,
    ) -> anyhow::Result<thread::ScopedJoinHandle<'scope, anyhow::Result<()>>> {
        thread::Builder::new()
            .name(format!("heartbeat({})", self.name))
            .spawn_scoped(scope, move || -> anyhow::Result<()> {
                let _s1 = span!(Level::INFO, "heartbeat", s = self.name, cid = conn_id).entered();

                loop {
                    trace!("checking stop_rx");
                    if stop.load(Ordering::Relaxed) {
                        info!("recvd stop msg");
                        return Ok(());
                    }

                    thread::sleep(consts::HEARTBEAT_DURATION);
                    {
                        let shell_to_client_ctl = self.shell_to_client_ctl.lock().unwrap();
                        match shell_to_client_ctl
                            .heartbeat
                            .send_timeout((), SHELL_TO_CLIENT_CTL_TIMEOUT)
                        {
                            // If the channel is disconnected, it means that the shell exited and
                            // the shell->client process exited cleanly. We should not raise a
                            // ruckus.
                            Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {
                                return Ok(())
                            }
                            // If we get a timeout it doesn't necessarily mean that the
                            // shell->client thread is unhealthy. It might just be busy
                            // doing other stuff. In particular, this comes up when the
                            // shell->client thread is generating a particularly large
                            // session restore buffer, which can take a minute.
                            Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {
                                continue;
                            }
                            _ => {}
                        }
                        let client_present = match shell_to_client_ctl
                            .heartbeat_ack
                            .recv_timeout(SHELL_TO_CLIENT_CTL_TIMEOUT)
                        {
                            // If the channel is disconnected, it means that the shell exited and
                            // the shell->client process exited cleanly. We should not raise a
                            // ruckus.
                            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return Ok(()),
                            Err(e) => return Err(e).context("waiting for heartbeat ack"),
                            Ok(client_present) => client_present,
                        };
                        if !client_present {
                            // Bail from the thread to get the rest of the
                            // client threads to clean themselves up.
                            return Ok(());
                        }
                    }
                }
            })
            .map_err(|e| anyhow!("{:?}", e))
    }

    #[instrument(skip_all)]
    fn spawn_supervisor<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        conn_id: usize,
        stop: &'scope AtomicBool,
        child_done: &'scope AtomicBool,
        pty_master: &'scope shpool_pty::fork::Master,
        child_exit_notifier: Arc<ExitNotifier>,
    ) -> anyhow::Result<thread::ScopedJoinHandle<'scope, anyhow::Result<()>>> {
        thread::Builder::new()
            .name(format!("supervisor({})", self.name))
            .spawn_scoped(scope, move || -> anyhow::Result<()> {
                let _s1 = span!(Level::INFO, "supervisor", s = self.name, cid = conn_id).entered();

                loop {
                    trace!("checking stop_rx (pty_master={:?})", pty_master.raw_fd());
                    if stop.load(Ordering::Relaxed) {
                        info!("recvd stop msg");
                        return Ok(());
                    }

                    match child_exit_notifier.wait(Some(SUPERVISOR_POLL_DUR)) {
                        Some(exit_status) => {
                            info!("child shell exited with status {}", exit_status);
                            // mark child as exited so the attach routine will
                            // cleanup correctly.
                            child_done.store(true, Ordering::Release);

                            // we don't need to worry about the ExitStatus frame
                            // because the shell->client thread cleanup should handle
                            // that.
                            return Ok(());
                        }
                        None => {
                            // shell is still running, do nothing
                            trace!("poll timeout");
                        }
                    }
                }
            })
            .map_err(|e| anyhow!("{:?}", e))
    }

    //
    // actions which can be bound to keybindings
    //

    #[instrument(skip_all)]
    fn action_detach(&self) -> anyhow::Result<()> {
        let shell_to_client_ctl = self.shell_to_client_ctl.lock().unwrap();
        shell_to_client_ctl
            .client_connection
            .send_timeout(ClientConnectionMsg::Disconnect, SHELL_TO_CLIENT_CTL_TIMEOUT)
            .context("signaling client detach to shell->client thread")?;
        let status = shell_to_client_ctl
            .client_connection_ack
            .recv_timeout(SHELL_TO_CLIENT_CTL_TIMEOUT)
            .context("waiting for client connection ack (3)")?;

        info!("action detach, status={:?}", status);
        Ok(())
    }
}

/// A handle for poking at the always-running shell->client thread.
/// Shared between the session struct (for calls originating with the cli)
/// and the session inner struct (for calls resulting from keybindings).
#[derive(Debug)]
pub struct ReaderCtl {
    /// A control channel for the shell->client thread. Whenever a new client
    /// dials in, the output stream for that client must be attached to the
    /// shell->client thread by sending it down this channel. A disconnect
    /// is signaled by sending None down this channel. Dropping the channel
    /// entirely causes the shell->client thread to exit.
    pub client_connection: crossbeam_channel::Sender<ClientConnectionMsg>,
    /// A control channel for the shell->client thread. Acks the addition of a
    /// fresh client connection.
    pub client_connection_ack: crossbeam_channel::Receiver<ClientConnectionStatus>,

    /// A control channel for the shell->client thread. Used to signal size
    /// changes so that the output spool will correctly reflect the size of
    /// the user's tty.
    pub tty_size_change: crossbeam_channel::Sender<TtySize>,
    /// A control channel for the shell->client thread. Acks the completion of a
    /// spool resize.
    pub tty_size_change_ack: crossbeam_channel::Receiver<()>,

    // A control channel telling the shell->client thread to issue
    // a heartbeat to check if the client is still listening.
    pub heartbeat: crossbeam_channel::Sender<()>,
    // True if the client is still listening, false if it has hung up
    // on us.
    pub heartbeat_ack: crossbeam_channel::Receiver<bool>,
}

/// Given a buffer, a length after which the data is not valid, a list of
/// sections to remove, and some scratch space, compact the given buffer and
/// return a new len.
///
/// The snip sections must all be within buf[..len], and must be
/// non-overlapping.
fn snip_buf(
    buf: &mut [u8],
    len: usize,
    snip_sections: &[(usize, usize)],        // (<len>, <end offset>)
    keep_sections: &mut Vec<(usize, usize)>, // re-usable scratch
) -> usize {
    if snip_sections.is_empty() {
        return len;
    }

    // build up the sections to keep in a more normal format
    keep_sections.clear();
    let mut cur_start = 0;
    for (len, end_offset) in snip_sections.iter() {
        let end_open = *end_offset + 1;
        let snip_start = end_open - len;
        if snip_start > cur_start {
            keep_sections.push((cur_start, snip_start));
        }
        cur_start = end_open;
    }
    keep_sections.push((cur_start, len));

    let mut last_end = 0;
    for (start, end) in keep_sections.iter() {
        if *start == *end {
            continue;
        }
        if *start == last_end {
            last_end = *end;
            continue;
        }
        let section_len = *end - *start;
        // Safety: we are copying sections of buf into itself, just overwriting
        //         little sections of the buffer. This should be fine because it
        //         is all happening within the same section of memory and
        //         std::ptr::copy (memmove in c) allows overlapping buffers.
        //         Also, these assertions should make it safer.
        assert!(last_end + section_len < buf.len());
        assert!(*start + section_len - 1 < buf.len());
        unsafe {
            std::ptr::copy(&buf[*start] as *const u8, &mut buf[last_end] as *mut u8, section_len);
        }
        last_end += section_len;
    }

    last_end
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_snip_buf() {
        let cases = vec![
            (vec![1, 1], 2, vec![(2, 1)], vec![]),
            (vec![1, 1, 3], 3, vec![(2, 1)], vec![3]),
            (vec![1, 1, 3, 4, 5], 5, vec![(2, 1), (1, 3)], vec![3, 5]),
            (vec![1, 1, 3, 4, 5, 8, 9, 1, 3], 5, vec![(2, 1), (1, 3)], vec![3, 5]),
            (vec![1, 1, 3, 4, 5, 8, 9, 1, 3], 9, vec![(5, 7)], vec![1, 1, 3, 3]),
        ];

        let mut keep_sections = vec![];
        for (mut buf, len, snips, want_buf) in cases.into_iter() {
            let got_len = snip_buf(&mut buf, len, &snips[..], &mut keep_sections);
            dbg!(got_len);
            assert_eq!(&buf[..got_len], &want_buf[..]);
        }
    }
}
