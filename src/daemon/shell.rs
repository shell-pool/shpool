use std::{
    io,
    io::{
        Read,
        Write,
    },
    os::unix::{
        io::AsRawFd,
        net::UnixStream,
    },
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
        Mutex,
        TryLockError,
    },
    thread,
    time,
};

use anyhow::{
    anyhow,
    Context,
};
use crossbeam_channel::RecvTimeoutError;
use tracing::{
    debug,
    error,
    info,
    instrument,
    span,
    trace,
    Level,
};

use crate::{
    consts,
    daemon::{
        config,
        keybindings,
    },
    protocol,
    test_hooks,
    tty,
};

const SUPERVISOR_POLL_DUR: time::Duration = time::Duration::from_millis(300);
const RPC_LOOP_POLL_DUR: time::Duration = time::Duration::from_millis(300);
const SESSION_MESSAGE_TIMEOUT: time::Duration = time::Duration::from_secs(10);

/// Session represent a shell session
#[derive(Debug)]
pub struct Session {
    pub started_at: time::SystemTime,
    pub rpc_in: crossbeam_channel::Sender<protocol::SessionMessageRequestPayload>,
    pub rpc_out: crossbeam_channel::Receiver<protocol::SessionMessageReply>,
    /// Mutable state with the lock held by the servicing handle_attach thread
    /// while a tty is attached to the session. Probing the mutex can be used
    /// to determine if someone is currently attached to the session.
    pub inner: Arc<Mutex<SessionInner>>,
}

impl Session {
    #[instrument(skip_all)]
    pub fn rpc_call(
        &self,
        arg: protocol::SessionMessageRequestPayload,
    ) -> anyhow::Result<protocol::SessionMessageReply> {
        // make a best effort attempt to avoid sending messages
        // to a session with no attached terminal
        match self.inner.try_lock() {
            // if it is locked, someone is attached
            Err(TryLockError::WouldBlock) => {},
            // if we can lock it, there is no one to get our msg
            _ => {
                return Ok(protocol::SessionMessageReply::NotAttached);
            },
        }

        self.rpc_in
            .send_timeout(arg, SESSION_MESSAGE_TIMEOUT)
            .context("sending session message")?;
        Ok(self
            .rpc_out
            .recv_timeout(SESSION_MESSAGE_TIMEOUT)
            .context("receiving session message reply")?)
    }
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
pub struct SessionInner {
    pub name: String, // to improve logging
    pub rpc_in: crossbeam_channel::Receiver<protocol::SessionMessageRequestPayload>,
    pub rpc_out: crossbeam_channel::Sender<protocol::SessionMessageReply>,
    pub child_exited: crossbeam_channel::Receiver<()>,
    pub pty_master: pty::fork::Fork,
    pub client_stream: Option<UnixStream>,
    pub config: config::Config,
}

impl SessionInner {
    #[instrument(skip_all)]
    pub fn handle_resize_rpc(
        &self,
        req: protocol::ResizeRequest,
    ) -> anyhow::Result<protocol::ResizeReply> {
        self.set_pty_size(&req.tty_size)?;
        Ok(protocol::ResizeReply::Ok)
    }

    pub fn set_pty_size(&self, size: &tty::Size) -> anyhow::Result<()> {
        let pty_master = self
            .pty_master
            .is_parent()
            .context("internal error: executing in child fork")?;
        size.set_fd(pty_master.as_raw_fd())
    }
}

impl SessionInner {
    /// bidi_stream shuffles bytes between the subprocess and
    /// the client connection. It returns true if the subprocess
    /// has exited, and false if it is still running.
    ///
    /// `spawned_handlers` is a channel that gets closed once all the threads
    /// for servicing the connection have been spawned. It should be a bounded
    /// unbuffered channel.
    #[instrument(skip_all, fields(s = self.name))]
    pub fn bidi_stream(
        &mut self,
        spawned_handlers: crossbeam_channel::Sender<()>,
    ) -> anyhow::Result<bool> {
        test_hooks::emit("daemon-bidi-stream-enter");
        let _bidi_stream_test_guard = test_hooks::scoped("daemon-bidi-stream-done");

        // we take the client stream so that it gets closed when this routine
        // returns
        let mut client_stream = match self.client_stream.take() {
            Some(s) => s,
            None => return Err(anyhow!("no client stream to take for bidi streaming")),
        };

        // set timeouts so we can wake up to handle cancelation correctly
        client_stream
            .set_nonblocking(true)
            .context("setting client stream nonblocking")?;

        let mut reader_client_stream = client_stream
            .try_clone()
            .context("creating reader client stream")?;
        let client_stream_m = Mutex::new(io::BufWriter::new(
            client_stream
                .try_clone()
                .context("wrapping stream in bufwriter")?,
        ));

        let pty_master = self
            .pty_master
            .is_parent()
            .context("internal error: executing in child fork")?;

        // A flag to indicate that outstanding threads should stop
        let stop = AtomicBool::new(false);
        // A flag to indicate if the child shell has exited
        let child_done = AtomicBool::new(false);

        thread::scope(|s| -> anyhow::Result<()> {
            // Spawn the main data transport threads
            let client_to_shell_h = self.spawn_client_to_shell(
                s, &stop, &pty_master, &mut reader_client_stream);
            let shell_to_client_h = self.spawn_shell_to_client(
                s, &stop, &pty_master, &client_stream_m, &mut client_stream);

            // Send a steady stream of heartbeats to the client
            // so that if the connection unexpectedly goes
            // down, we detect it immediately.
            let heartbeat_h = self.spawn_heartbeat(
                s, &stop, &client_stream_m);

            // poll the pty master fd to see if the child
            // shell has exited.
            let supervisor_h = self.spawn_supervisor(
                s, &stop, &child_done, &pty_master);

            // handle SessionMessage RPCs
            let rpc_h = self.spawn_rpc(s, &stop);

            drop(spawned_handlers);

            loop {
                let c_done = child_done.load(Ordering::Acquire);
                if client_to_shell_h.is_finished() || shell_to_client_h.is_finished()
                    || heartbeat_h.is_finished() || supervisor_h.is_finished() || rpc_h.is_finished() || c_done {
                    debug!("s({}): bidi_stream: signaling for threads to stop: client_to_shell_finished={} shell_to_client_finished={} heartbeat_finished={} supervisor_finished={} rpc_finished={} child_done={}",
                        self.name,
                        client_to_shell_h.is_finished(),
                        shell_to_client_h.is_finished(),
                        heartbeat_h.is_finished(),
                        supervisor_h.is_finished(),
                        rpc_h.is_finished(),
                        c_done,
                    );
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                thread::sleep(consts::JOIN_POLL_DURATION);
            }
            debug!("s({}): bidi_stream: joining client_to_shell_h", self.name);
            match client_to_shell_h.join() {
                Ok(v) => v.context("joining client_to_shell_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            debug!("s({}): bidi_stream: joining shell_to_client_h", self.name);
            match shell_to_client_h.join() {
                Ok(v) => v.context("joining shell_to_client_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            debug!("s({}): bidi_stream: joining heartbeat_h", self.name);
            match heartbeat_h.join() {
                Ok(v) => v.context("joining heartbeat_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            debug!("s({}): bidi_stream: joining supervisor_h", self.name);
            match supervisor_h.join() {
                Ok(v) => v.context("joining supervisor_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }

            Ok(())
        }).context("outer thread scope")?;

        let c_done = child_done.load(Ordering::Acquire);
        if c_done {
            client_stream
                .shutdown(std::net::Shutdown::Both)
                .context("shutting down client stream")?;
        }

        info!("s({}): bidi_stream: done child_done={}", self.name, c_done);
        Ok(c_done)
    }

    #[instrument(skip_all)]
    fn spawn_client_to_shell<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
        pty_master: &'scope pty::fork::Master,
        reader_client_stream: &'scope mut UnixStream,
    ) -> thread::ScopedJoinHandle<anyhow::Result<()>> {
        let empty_bindings = vec![];
        let bindings = keybindings::Bindings::new(
            self.config
                .keybindings
                .as_ref()
                .unwrap_or(&empty_bindings)
                .iter()
                .map(|binding| (binding.binding.as_str(), binding.action)),
        );

        scope.spawn(|| -> anyhow::Result<()> {
            let _s = span!(Level::INFO, "client->shell", s = self.name).entered();
            let mut bindings = bindings.context("compiling keybindings engine")?;

            let mut master_writer = pty_master.clone();

            info!("spawned");

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if stop.load(Ordering::Relaxed) {
                    info!("recvd stop msg (1)");
                    return Ok(());
                }

                // N.B. we don't need to muck about with chunking or anything
                // in this direction, because there is only one input stream
                // to the shell subprocess, vs the two output streams we need
                // to handle coming back the other way.
                //
                // Also, note that we don't access through the mutex because reads
                // don't need to be excluded from trampling on writes.
                let len = match reader_client_stream.read(&mut buf) {
                    Ok(l) => l,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        trace!("read: WouldBlock");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    },
                    Err(e) => return Err(e).context("reading client chunk"),
                };
                if len == 0 {
                    continue;
                }
                test_hooks::emit("daemon-read-c2s-chunk");

                debug!("read {} bytes", len);

                let mut to_write = &buf[0..len];
                debug!("created to_write='{}'", String::from_utf8_lossy(to_write));

                while to_write.len() > 0 {
                    if stop.load(Ordering::Relaxed) {
                        info!("recvd stop msg (2)");
                        return Ok(());
                    }

                    let nwritten = match master_writer.write(&to_write) {
                        Ok(n) => n,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            trace!("read: WouldBlock");
                            thread::sleep(consts::PIPE_POLL_DURATION);
                            continue;
                        },
                        Err(e) => return Err(e).context("writing client chunk"),
                    };
                    debug!("wrote {} bytes", nwritten);
                    to_write = &to_write[nwritten..];
                    trace!("to_write='{}'", String::from_utf8_lossy(to_write));
                }

                // TODO(ethan): perform keybinding scanning in a background
                //              thread
                for byte in buf[0..len].into_iter() {
                    if let Some(action) = bindings.transition(*byte) {
                        info!("keybinding for {:?} fired", action);
                    }
                }

                master_writer
                    .flush()
                    .context("flushing input from client to shell")?;

                debug!("flushed chunk of len {}", len);
            }
        })
    }

    #[instrument(skip_all)]
    fn spawn_shell_to_client<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
        pty_master: &'scope pty::fork::Master,
        client_stream_m: &'scope Mutex<io::BufWriter<UnixStream>>,
        client_stream: &'scope mut UnixStream,
    ) -> thread::ScopedJoinHandle<anyhow::Result<()>> {
        scope.spawn(move || -> anyhow::Result<()> {
            let _s1 = span!(Level::INFO, "shell->client", s = self.name).entered();

            info!("spawned");

            let mut master_reader = pty_master.clone();

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if stop.load(Ordering::Relaxed) {
                    info!("recvd stop msg");
                    return Ok(());
                }

                // select so we know which stream to read from, and
                // know to wake up immediately when bytes are available.
                let mut fdset = nix::sys::select::FdSet::new();
                fdset.insert(master_reader.as_raw_fd());
                let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                let nready = match nix::sys::select::select(
                    None,
                    Some(&mut fdset),
                    None,
                    None,
                    Some(&mut poll_dur),
                ) {
                    Ok(n) => n,
                    Err(nix::errno::Errno::EBADF) => {
                        info!("shell went down");
                        return Ok(());
                    },
                    Err(e) => return Err(e).context("selecting on pty master"),
                };
                if nready == 0 {
                    continue;
                }

                if fdset.contains(master_reader.as_raw_fd()) {
                    let len = match master_reader.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            trace!("pty master read: WouldBlock");
                            thread::sleep(consts::PIPE_POLL_DURATION);
                            continue;
                        },
                        Err(e) => {
                            info!("error reading from pty: {:?}", e);
                            return Err(e).context("reading pty master chunk");
                        },
                    };
                    if len == 0 {
                        trace!("0 stdout bytes, waiting");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    }

                    let chunk = protocol::Chunk {
                        kind: protocol::ChunkKind::Data,
                        buf: &buf[..len],
                    };
                    debug!(
                        "read pty master len={} '{}'",
                        len,
                        String::from_utf8_lossy(chunk.buf)
                    );
                    {
                        let mut s = client_stream_m.lock().unwrap();
                        chunk
                            .write_to(&mut *s, &stop)
                            .and_then(|_| s.flush())
                            .context("writing stdout chunk to client stream")?;
                    }
                    debug!("wrote {} pty master bytes", chunk.buf.len());
                    test_hooks::emit("daemon-wrote-s2c-chunk");
                }

                // flush immediately
                client_stream.flush().context("flushing client stream")?;
            }
        })
    }

    #[instrument(skip_all)]
    fn spawn_heartbeat<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
        client_stream_m: &'scope Mutex<io::BufWriter<UnixStream>>,
    ) -> thread::ScopedJoinHandle<anyhow::Result<()>> {
        scope.spawn(move || -> anyhow::Result<()> {
            let _s1 = span!(Level::INFO, "heartbeat", s = self.name).entered();

            loop {
                trace!("checking stop_rx");
                if stop.load(Ordering::Relaxed) {
                    info!("recvd stop msg");
                    return Ok(());
                }

                thread::sleep(consts::HEARTBEAT_DURATION);
                let chunk = protocol::Chunk {
                    kind: protocol::ChunkKind::Heartbeat,
                    buf: &[],
                };
                {
                    let mut s = client_stream_m.lock().unwrap();
                    match chunk.write_to(&mut *s, &stop).and_then(|_| s.flush()) {
                        Ok(_) => {
                            trace!("wrote heartbeat");
                        },
                        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                            trace!("client hangup");
                            return Ok(());
                        },
                        Err(e) => {
                            return Err(e).context("writing heartbeat")?;
                        },
                    }
                }
            }
        })
    }

    #[instrument(skip_all)]
    fn spawn_supervisor<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
        child_done: &'scope AtomicBool,
        pty_master: &'scope pty::fork::Master,
    ) -> thread::ScopedJoinHandle<anyhow::Result<()>> {
        scope.spawn(|| -> anyhow::Result<()> {
            let _s1 = span!(Level::INFO, "supervisor", s = self.name).entered();

            loop {
                trace!("checking stop_rx (pty_master={})", pty_master.as_raw_fd());
                if stop.load(Ordering::Relaxed) {
                    info!("recvd stop msg");
                    return Ok(());
                }

                match self.child_exited.recv_timeout(SUPERVISOR_POLL_DUR) {
                    Ok(_) => {
                        error!("internal error: unexpected send on child_exited chan");
                    },
                    Err(RecvTimeoutError::Timeout) => {
                        // shell is still running, do nothing
                        trace!("poll timeout");
                    },
                    Err(RecvTimeoutError::Disconnected) => {
                        info!("child shell exited");
                        child_done.store(true, Ordering::Release);
                        return Ok(());
                    },
                }
            }
        })
    }

    #[instrument(skip_all)]
    fn spawn_rpc<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
    ) -> thread::ScopedJoinHandle<anyhow::Result<()>> {
        scope.spawn(|| -> anyhow::Result<()> {
            let _s1 = span!(Level::INFO, "rpc", s = self.name).entered();

            loop {
                if stop.load(Ordering::Relaxed) {
                    info!("recvd stop msg");
                    return Ok(());
                }

                let req = match self.rpc_in.recv_timeout(RPC_LOOP_POLL_DUR) {
                    Ok(r) => r,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(e) => Err(e).context("recving sessession msg")?,
                };
                let resp = match req {
                    protocol::SessionMessageRequestPayload::Resize(req) => {
                        debug!("handling resize");
                        protocol::SessionMessageReply::Resize(self.handle_resize_rpc(req)?)
                    },
                    protocol::SessionMessageRequestPayload::Detach => {
                        debug!("handling detach");
                        stop.store(true, Ordering::Relaxed);
                        protocol::SessionMessageReply::Detach(
                            protocol::SessionMessageDetachReply::Ok,
                        )
                    },
                };

                // A timeout here is a hard error because it represents
                // lost data. We could technically write a retry loop
                // around the timeout, but it is an unbounded channel,
                // so a timeout seems very unlikely.
                self.rpc_out
                    .send_timeout(resp, RPC_LOOP_POLL_DUR)
                    .context("sending session reply")?
            }
        })
    }
}
