use std::{
    io,
    io::{Read, Write},
    net,
    ops::Add,
    os::unix::{io::AsRawFd, net::UnixStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread, time,
};

use anyhow::{anyhow, Context};
use crossbeam_channel::RecvTimeoutError;
use tracing::{debug, error, info, instrument, span, trace, warn, Level};

use crate::{
    consts,
    daemon::{config, keybindings},
    protocol, test_hooks, tty,
};

const SUPERVISOR_POLL_DUR: time::Duration = time::Duration::from_millis(300);

// Chosen experimentally. This value is small enough that no human will likely
// recognize it, and it seems to be large enough that emacs consistently picks
// up the "jiggle" trick where we oversize the pty then put it back to the right
// size.
const REATTACH_RESIZE_DELAY: time::Duration = time::Duration::from_millis(50);

// The reader thread should wake up relatively frequently so it can detect
// reattach, but we don't need to go crazy since reattach is not part of
// the inner loop.
const READER_POLL_MS: libc::c_int = 100;

/// Session represent a shell session
#[derive(Debug)]
pub struct Session {
    pub started_at: time::SystemTime,
    pub reader_ctl: Arc<Mutex<ReaderCtl>>,
    /// Mutable state with the lock held by the servicing handle_attach thread
    /// while a tty is attached to the session. Probing the mutex can be used
    /// to determine if someone is currently attached to the session.
    pub inner: Arc<Mutex<SessionInner>>,
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
pub struct SessionInner {
    pub name: String, // to improve logging
    pub reader_ctl: Arc<Mutex<ReaderCtl>>,
    pub child_exited: crossbeam_channel::Receiver<()>,
    pub pty_master: pty::fork::Fork,
    pub client_stream: Option<UnixStream>,
    pub config: config::Config,

    /// The join handle for the always-on background reader thread.
    /// Only wrapped in an option so we can spawn the thread after
    /// constructing the SessionInner.
    pub reader_join_h: Option<thread::JoinHandle<anyhow::Result<()>>>,
}

/// A notification that a new client has connected, sent to the
/// reader thread.
pub struct ClientConnection {
    /// All output data should be written to this sink rather than
    /// directly to the unix stream. The mutex makes sure that we don't
    /// accidentally interleave with heartbeat frames.
    sink: Arc<Mutex<io::BufWriter<UnixStream>>>,
    /// The size of the client tty.
    size: tty::Size,
    /// The raw unix socket stream. The reader should never write
    /// to this directly, just use it for control operations like
    /// shutdown.
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
    size: tty::Size,
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

impl SessionInner {
    /// Spawn the reader thread which continually reads from the pty
    /// and sends data both to the output spool and to the client,
    /// if one is attached.
    #[instrument(skip_all, fields(s = self.name))]
    pub fn spawn_reader(
        &self,
        tty_size: tty::Size,
        scrollback_lines: usize,
        session_restore_mode: config::SessionRestoreMode,
        client_connection: crossbeam_channel::Receiver<Option<ClientConnection>>,
        client_connection_ack: crossbeam_channel::Sender<ClientConnectionStatus>,
        tty_size_change: crossbeam_channel::Receiver<tty::Size>,
        tty_size_change_ack: crossbeam_channel::Sender<()>,
    ) -> anyhow::Result<thread::JoinHandle<anyhow::Result<()>>> {
        use nix::poll;

        let mut pty_master = self.pty_master.is_parent()?.clone();
        let name = self.name.clone();
        let mut closure = move || {
            let _s = span!(Level::INFO, "reader", s = name).entered();

            let mut output_spool =
                vt100::Parser::new(tty_size.rows, tty_size.cols, scrollback_lines);
            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];
            let mut poll_fds = [poll::PollFd::new(pty_master.as_raw_fd(), poll::PollFlags::POLLIN)];

            // block until we get the first connection attached so that we don't drop
            // the initial prompt on the floor
            info!("waiting for initial client connection");
            let mut client_conn: Option<ClientConnection> =
                client_connection.recv().context("waiting for initial client connection")?;
            client_connection_ack
                .send(ClientConnectionStatus::New)
                .context("sending initial client connection ack")?;
            info!("got initial client connection");

            let mut resize_cmd: Option<ResizeCmd> = client_conn.as_ref().and_then(|conn| {
                Some(ResizeCmd { size: conn.size.clone(), when: time::Instant::now() })
            });

            loop {
                let mut do_reattach = false;
                crossbeam_channel::select! {
                    recv(client_connection) -> new_connection => {
                        match new_connection {
                            Ok(Some(conn)) => {
                                do_reattach = true;
                                let ack = if client_conn.is_some() {
                                    ClientConnectionStatus::Replaced
                                } else {
                                    ClientConnectionStatus::New
                                };
                                // Resize the pty to be bigger than it needs to be,
                                // we do this immediately so that the extra size
                                // can "bake" for a little bit, which emacs seems
                                // to require in order to pick up the jiggle.
                                let oversize = tty::Size {
                                    rows: conn.size.rows + 1,
                                    cols: conn.size.cols + 1,
                                };
                                oversize.set_fd(pty_master.as_raw_fd())?;

                                // Always instantly resize the spool, since we don't
                                // need to inject a delay into that.
                                output_spool.screen_mut().set_size(
                                    conn.size.rows, conn.size.cols);
                                resize_cmd = Some(ResizeCmd {
                                    size: conn.size.clone(),
                                    when: time::Instant::now().add(REATTACH_RESIZE_DELAY),
                                });
                                client_conn = Some(conn);

                                client_connection_ack.send(ack)
                                    .context("sending client connection ack")?;
                            }
                            Ok(None) => {
                                let ack = if let Some(old_conn) = client_conn {
                                    old_conn.stream.shutdown(net::Shutdown::Both)?;
                                    ClientConnectionStatus::Detached
                                } else {
                                    ClientConnectionStatus::DetachNone
                                };
                                client_conn = None;

                                client_connection_ack.send(ack)
                                    .context("sending client connection ack")?;
                            }

                            // SessionInner getting dropped, so this thread should go away.
                            Err(crossbeam_channel::RecvError) => return Ok(()),
                        }
                    }
                    recv(tty_size_change) -> new_size => {
                        if let Ok(size) = new_size {
                            info!("resize size={:?}", size);
                            output_spool.screen_mut()
                                .set_size(size.rows, size.cols);
                            resize_cmd = Some(ResizeCmd {
                                size: size,
                                // No delay needed for ordinary resizes, just
                                // for reconnects.
                                when: time::Instant::now(),
                            });
                            tty_size_change_ack.send(())
                                .context("sending size change ack")?;
                        }
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
                        resize_cmd.size.set_fd(pty_master.as_raw_fd())?;
                        executed_resize = true;
                    }
                }
                if executed_resize {
                    resize_cmd = None;
                }

                if do_reattach {
                    use config::SessionRestoreMode::*;

                    info!("executing reattach protocol (mode={:?})", session_restore_mode);
                    let restore_buf = match session_restore_mode {
                        Simple => vec![],
                        Screen => output_spool.screen().contents_formatted(),
                        Lines(nlines) => {
                            output_spool.screen().last_n_rows_contents_formatted(nlines)
                        }
                    };
                    if let (true, Some(conn)) = (restore_buf.len() > 0, client_conn.as_ref()) {
                        trace!("restore chunk='{}'", String::from_utf8_lossy(&restore_buf[..]));
                        let chunk = protocol::Chunk {
                            kind: protocol::ChunkKind::Data,
                            buf: &restore_buf[..],
                        };

                        let mut s = conn.sink.lock().unwrap();
                        if let Err(err) = chunk.write_to(&mut *s).and_then(|_| s.flush()) {
                            warn!("write err session-restor buf: {:?}", err);
                        }
                    }
                }

                // Block until the shell has some data for us so we can be sure our reads
                // always succeed. We don't want to end up blocked forever on a read while
                // a client is trying to attach.
                let nready = poll::poll(&mut poll_fds, READER_POLL_MS)?;
                if nready == 0 {
                    // if timeout
                    continue;
                }
                if nready != 1 {
                    return Err(anyhow!("reader thread: expected exactly 1 ready fd"));
                }
                let len = pty_master.read(&mut buf).context("reading pty master chunk")?;
                trace!("read pty master len={} '{}'", len, String::from_utf8_lossy(&buf[..len]));
                if len == 0 {
                    continue;
                }

                output_spool.process(&buf[..len]);

                let mut reset_client_conn = false;
                if let Some(conn) = client_conn.as_ref() {
                    let chunk =
                        protocol::Chunk { kind: protocol::ChunkKind::Data, buf: &buf[..len] };
                    let mut s = conn.sink.lock().unwrap();
                    let write_result = chunk.write_to(&mut *s).and_then(|_| s.flush());
                    if let Err(err) = write_result {
                        info!("client_stream write err, assuming hangup: {:?}", err);
                        reset_client_conn = true;
                    } else {
                        test_hooks::emit("daemon-wrote-s2c-chunk");
                    }
                }
                if reset_client_conn {
                    client_conn = None;
                }
            }
        };

        Ok(thread::spawn(move || log_if_error("error in reader", closure())))
    }

    /// bidi_stream shuffles bytes between the subprocess and
    /// the client connection. It returns true if the subprocess
    /// has exited, and false if it is still running.
    #[instrument(skip_all, fields(s = self.name))]
    pub fn bidi_stream(&mut self, init_tty_size: tty::Size) -> anyhow::Result<bool> {
        test_hooks::emit("daemon-bidi-stream-enter");
        let _bidi_stream_test_guard = test_hooks::scoped("daemon-bidi-stream-done");

        // we take the client stream so that it gets closed when this routine
        // returns
        let client_stream = match self.client_stream.take() {
            Some(s) => s,
            None => return Err(anyhow!("no client stream to take for bidi streaming")),
        };

        let mut client_to_shell_client_stream =
            client_stream.try_clone().context("creating reader client stream")?;
        let closable_client_stream =
            client_stream.try_clone().context("creating closable client stream handle")?;
        let reader_client_stream =
            client_stream.try_clone().context("creating reader client stream handle")?;
        let client_stream_m = Arc::new(Mutex::new(io::BufWriter::new(
            client_stream.try_clone().context("wrapping stream in bufwriter")?,
        )));

        {
            let reader_ctl = self.reader_ctl.lock().unwrap();
            reader_ctl
                .client_connection
                .send(Some(ClientConnection {
                    sink: Arc::clone(&client_stream_m),
                    size: init_tty_size,
                    stream: reader_client_stream,
                }))
                .context("attaching new client stream to reader thread")?;
            let status = reader_ctl
                .client_connection_ack
                .recv()
                .context("waiting for client connection ack")?;
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
                s, &stop, &pty_master, &mut client_to_shell_client_stream);

            // Send a steady stream of heartbeats to the client
            // so that if the connection unexpectedly goes
            // down, we detect it immediately.
            let heartbeat_h = self.spawn_heartbeat(
                s, &stop, &client_stream_m);

            // poll the pty master fd to see if the child
            // shell has exited.
            let supervisor_h = self.spawn_supervisor(
                s, &stop, &child_done, &pty_master);

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

            {
                let reader_ctl = self.reader_ctl.lock().unwrap();
                reader_ctl.client_connection.send(None)
                    .context("signaling client detach to reader thread")?;
                let status = reader_ctl.client_connection_ack.recv()
                    .context("waiting for client connection ack")?;
                info!("detached from reader, status = {:?}", status);
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
                let mut len =
                    reader_client_stream.read(&mut buf).context("reading client chunk")?;
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
                        }
                        NoMatch => {
                            partial_keybinding.clear();
                        }
                        Partial => partial_keybinding.push(*byte),
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
                if partial_keybinding.len() > 0 {
                    // we have a partial keybinding pending, so don't write
                    // it to the output stream immediately
                    let snip_chunk_len =
                        if partial_keybinding.len() > len { len } else { partial_keybinding.len() };
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
    }

    #[instrument(skip_all)]
    fn spawn_heartbeat<'scope>(
        &'scope self,
        scope: &'scope thread::Scope<'scope, '_>,
        stop: &'scope AtomicBool,
        client_stream_m: &'scope Arc<Mutex<io::BufWriter<UnixStream>>>,
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
                let chunk = protocol::Chunk { kind: protocol::ChunkKind::Heartbeat, buf: &[] };
                {
                    let mut s = client_stream_m.lock().unwrap();
                    match chunk.write_to(&mut *s).and_then(|_| s.flush()) {
                        Ok(_) => {
                            trace!("wrote heartbeat");
                        }
                        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                            trace!("client hangup");
                            return Ok(());
                        }
                        Err(e) => {
                            return Err(e).context("writing heartbeat")?;
                        }
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
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        // shell is still running, do nothing
                        trace!("poll timeout");
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        info!("child shell exited");
                        child_done.store(true, Ordering::Release);
                        return Ok(());
                    }
                }
            }
        })
    }

    //
    // actions which can be bound to keybindings
    //

    #[instrument(skip_all)]
    fn action_detach(&self) -> anyhow::Result<()> {
        let reader_ctl = self.reader_ctl.lock().unwrap();
        reader_ctl
            .client_connection
            .send(None)
            .context("signaling client detach to reader thread")?;
        let status =
            reader_ctl.client_connection_ack.recv().context("waiting for client connection ack")?;

        info!("action detach, status={:?}", status);
        Ok(())
    }
}

/// A handle for poking at the always-running reader thread.
/// Shared between the session struct (for calls originating with the cli)
/// and the session inner struct (for calls resulting from keybindings).
#[derive(Debug)]
pub struct ReaderCtl {
    /// A control channel for the reader thread. Whenever a new client dials in,
    /// the output stream for that client must be attached to the reader
    /// thread by sending it down this channel. A disconnect is signaled by
    /// sending None down this channel. Dropping the channel entirely causes
    /// the reader thread to exit.
    pub client_connection: crossbeam_channel::Sender<Option<ClientConnection>>,
    /// A control channel for the reader thread. Acks the addition of a fresh
    /// client connection.
    pub client_connection_ack: crossbeam_channel::Receiver<ClientConnectionStatus>,

    /// A control channel for the reader thread. Used to signal size changes so
    /// that the output spool will correctly reflect the size of the user's
    /// tty.
    pub tty_size_change: crossbeam_channel::Sender<tty::Size>,
    /// A control channel for the reader thread. Acks the completion of a spool
    /// resize.
    pub tty_size_change_ack: crossbeam_channel::Receiver<()>,
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
