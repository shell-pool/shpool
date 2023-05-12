use std::{
    io,
    io::{
        Read,
        Write,
    },
    net,
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
    pub caller: Arc<Mutex<SessionCaller>>,
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

        let caller = self.caller.lock().unwrap();
        caller.call(arg)
    }
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
pub struct SessionInner {
    pub name: String, // to improve logging
    pub caller: Arc<Mutex<SessionCaller>>,
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

        let mut reader_client_stream = client_stream
            .try_clone()
            .context("creating reader client stream")?;
        let closable_client_stream = client_stream
            .try_clone()
            .context("creating closable client stream handle")?;
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
            let rpc_h = self.spawn_rpc(s, &stop, &closable_client_stream);

            drop(spawned_handlers);

            loop {
                let c_done = child_done.load(Ordering::Acquire);
                if client_to_shell_h.is_finished() || shell_to_client_h.is_finished()
                    || heartbeat_h.is_finished() || supervisor_h.is_finished() || rpc_h.is_finished() || c_done {
                    debug!("signaling for threads to stop: client_to_shell_finished={} shell_to_client_finished={} heartbeat_finished={} supervisor_finished={} rpc_finished={} child_done={}",
                        client_to_shell_h.is_finished(),
                        shell_to_client_h.is_finished(),
                        heartbeat_h.is_finished(),
                        supervisor_h.is_finished(),
                        rpc_h.is_finished(),
                        c_done,
                    );
                    stop.store(true, Ordering::Relaxed);
                    closable_client_stream.shutdown(net::Shutdown::Both)?;
                    break;
                }
                thread::sleep(consts::JOIN_POLL_DURATION);
            }
            debug!("joining client_to_shell_h");
            match client_to_shell_h.join() {
                Ok(v) => v.context("joining client_to_shell_h")?,
                Err(panic_err) => {
                    debug!("client_to_shell panic_err = {:?}", panic_err);
                    std::panic::resume_unwind(panic_err)
                },
            }
            debug!("joining shell_to_client_h");
            match shell_to_client_h.join() {
                Ok(v) => v.context("joining shell_to_client_h")?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
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

            Ok(())
        }).context("outer thread scope")?;

        let c_done = child_done.load(Ordering::Acquire);
        if c_done {
            client_stream
                .shutdown(std::net::Shutdown::Both)
                .context("shutting down client stream")?;
        }

        info!("bidi_stream: done child_done={}", c_done);
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
        let empty_bindings = vec![config::Keybinding {
            binding: String::from("Ctrl-Space Ctrl-q"),
            action: keybindings::Action::Detach,
        }];
        let bindings = keybindings::Bindings::new(
            self.config
                .keybinding
                .as_ref()
                .unwrap_or(&empty_bindings)
                .iter()
                .map(|binding| (binding.binding.as_str(), binding.action)),
        );

        scope.spawn(|| -> anyhow::Result<()> {
            let _s = span!(Level::INFO, "client->shell", s = self.name).entered();
            let mut bindings = bindings.context("compiling keybindings engine")?;

            let mut master_writer = pty_master.clone();

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
                // to the shell subprocess, vs the two output streams we need
                // to handle coming back the other way.
                //
                // Also, note that we don't access through the mutex because reads
                // don't need to be excluded from trampling on writes.
                let mut len = reader_client_stream
                    .read(&mut buf)
                    .context("reading client chunk")?;
                if len == 0 {
                    continue;
                }
                test_hooks::emit("daemon-read-c2s-chunk");
                trace!(
                    "read client len={}: '{}'",
                    len,
                    String::from_utf8_lossy(&buf[..len]),
                );

                // We might be able to gain some perf by doing this scanning in
                // a background thread (though maybe not given the need to copy
                // the data), but just doing it inline doesn't seem have have
                // a major perf impact, and this way is simpler.
                snip_sections.clear();
                for (i, byte) in buf[0..len].into_iter().enumerate() {
                    use keybindings::BindingResult::*;
                    match bindings.transition(*byte) {
                        NoMatch if partial_keybinding.len() > 0 && i < partial_keybinding.len() => {
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
                        },
                        NoMatch => {
                            partial_keybinding.clear();
                        },
                        Partial => partial_keybinding.push(*byte),
                        Match(action) => {
                            info!("{:?} keybinding action fired", action);
                            let keybinding_len = partial_keybinding.len() + 1;
                            if keybinding_len < i {
                                // this keybinding is wholely contained in buf
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
                                NoOp => {},
                            }
                        },
                    }
                }
                if partial_keybinding.len() > 0 {
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

                master_writer
                    .write_all(&buf[0..len])
                    .context("writing client chunk")?;

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

                // select so we only perform reads that will succeed, avoiding deadlocks.
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
                let len = master_reader
                    .read(&mut buf)
                    .context("reading pty master chunk")?;
                let chunk = protocol::Chunk {
                    kind: protocol::ChunkKind::Data,
                    buf: &buf[..len],
                };
                trace!(
                    "read pty master len={} '{}'",
                    len,
                    String::from_utf8_lossy(chunk.buf),
                );
                {
                    let mut s = client_stream_m.lock().unwrap();
                    chunk
                        .write_to(&mut *s)
                        .and_then(|_| s.flush())
                        .context("writing stdout chunk to client stream")?;
                }
                debug!("wrote {} pty master bytes", chunk.buf.len());
                test_hooks::emit("daemon-wrote-s2c-chunk");

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
                    match chunk.write_to(&mut *s).and_then(|_| s.flush()) {
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
        client_stream: &'scope UnixStream,
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
                        protocol::SessionMessageReply::Resize(match self.handle_resize_rpc(req) {
                            Ok(_) => protocol::ResizeReply::Ok,
                            Err(err) => {
                                // only log about resize errors since they seem to happen in
                                // headless test environments, but we don't actually care in
                                // such situations
                                error!("resize failed: {:?}", err);
                                protocol::ResizeReply::Failed
                            },
                        })
                    },
                    protocol::SessionMessageRequestPayload::Detach => {
                        debug!("handling detach");
                        stop.store(true, Ordering::Relaxed);
                        client_stream.shutdown(net::Shutdown::Both)?;
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

    //
    // actions which can be bound to keybindings
    //

    #[instrument(skip_all)]
    fn action_detach(&self) -> anyhow::Result<()> {
        let caller = self.caller.lock().unwrap();
        let reply = caller.call(protocol::SessionMessageRequestPayload::Detach)?;
        info!("action detach, reply={:?}", reply);
        Ok(())
    }
}

/// A handle for making calls to the rpc handler thread for an active session.
/// Shared between the session struct (for calls originating with the cli)
/// and the session inner struct (for calls resulting from keybindings).
#[derive(Debug)]
pub struct SessionCaller {
    pub rpc_in: crossbeam_channel::Sender<protocol::SessionMessageRequestPayload>,
    pub rpc_out: crossbeam_channel::Receiver<protocol::SessionMessageReply>,
}

impl SessionCaller {
    #[instrument(skip_all)]
    pub fn call(
        &self,
        arg: protocol::SessionMessageRequestPayload,
    ) -> anyhow::Result<protocol::SessionMessageReply> {
        self.rpc_in
            .send_timeout(arg, SESSION_MESSAGE_TIMEOUT)
            .context("sending session message")?;
        Ok(self
            .rpc_out
            .recv_timeout(SESSION_MESSAGE_TIMEOUT)
            .context("receiving session message reply")?)
    }
}

/// Given a buffer, a length after which the data is not valid, a list of
/// sections to remove, and some scratch space, compact the given buffer and
/// return a new len.
///
/// The snip sections must all be within buf[..len], and must be non-overlapping.
fn snip_buf(
    buf: &mut [u8],
    len: usize,
    snip_sections: &[(usize, usize)],        // (<len>, <end offset>)
    keep_sections: &mut Vec<(usize, usize)>, // re-usable scratch
) -> usize {
    if snip_sections.len() == 0 {
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
        // Saftey: we are copying sections of buf into itself, just overwriting
        //         little sections of the buffer. This should be fine because it
        //         is all happening within the same section of memory and
        //         std::ptr::copy (memmove in c). Also, these assertions should
        //         make it safer.
        assert!(last_end + section_len < buf.len());
        assert!(*start + section_len - 1 < buf.len());
        unsafe {
            std::ptr::copy(
                &buf[*start] as *const u8,
                &mut buf[last_end] as *mut u8,
                section_len,
            );
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
            (
                vec![1, 1, 3, 4, 5, 8, 9, 1, 3],
                5,
                vec![(2, 1), (1, 3)],
                vec![3, 5],
            ),
            (
                vec![1, 1, 3, 4, 5, 8, 9, 1, 3],
                9,
                vec![(5, 7)],
                vec![1, 1, 3, 3],
            ),
        ];

        let mut keep_sections = vec![];
        for (mut buf, len, snips, want_buf) in cases.into_iter() {
            let got_len = snip_buf(&mut buf, len, &snips[..], &mut keep_sections);
            dbg!(got_len);
            assert_eq!(&buf[..got_len], &want_buf[..]);
        }
    }
}
