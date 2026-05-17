//! Push-event protocol for the daemon.
//!
//! Events are published to subscribers connected to a sibling Unix socket
//! next to the main shpool socket. The wire format is JSON, one event per
//! line (newline-delimited; aka JSONL). Non-Rust clients only need a Unix
//! socket and a JSON parser to consume the stream. Literal newlines inside
//! a JSON value are not possible: RFC 8259 §7 requires control characters
//! (including U+000A LINE FEED) to be escaped inside strings, so framing
//! by `\n` is unambiguous.
//!
//! Events carry no payload beyond their type — they signal that *something*
//! changed in the session table. Subscribers learn the new state by calling
//! `shpool list` (or the equivalent over the main socket). Subscribers that
//! fall too far behind are dropped and may simply reconnect.
//!
//! Architecture: a single `events-sink` thread owns all subscriber state and
//! does all I/O via non-blocking `poll(2)`. `publish()` is O(1) on the
//! daemon's hot path: a `try_send` on a bounded channel + a 1-byte write to
//! a self-pipe to wake the sink.

use std::{
    collections::VecDeque,
    io::{self, Write},
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, LazyLock,
    },
    thread,
};

use anyhow::Context;
use nix::{
    errno::Errno,
    poll::{self, PollFd, PollFlags, PollTimeout},
    unistd,
};
use serde_derive::Serialize;
use tracing::{error, info, warn};

/// Per-subscriber outbound queue depth (events). Subscribers that fall this
/// far behind are dropped and must reconnect.
const SUBSCRIBER_QUEUE_DEPTH: usize = 64;

/// Capacity of the publish-to-sink channel. Reaching it means the sink is
/// wedged -- a real bug, not a tunable.
const EVENT_CHANNEL_CAP: usize = 4096;

/// An event published on the events socket.
#[derive(Serialize, Debug)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum Event {
    #[serde(rename = "session.created")]
    SessionCreated,
    #[serde(rename = "session.attached")]
    SessionAttached,
    #[serde(rename = "session.detached")]
    SessionDetached,
    #[serde(rename = "session.removed")]
    SessionRemoved,
}

/// The publish surface of the events system: fans out events to all
/// connected subscribers via a single background sink thread. Freely
/// cloneable (each clone shares the one sink). The sink's lifetime is
/// owned by the [`EventBusHandle`] returned alongside this from
/// [`EventBus::start`]; dropping that handle stops and joins the sink.
pub struct EventBus {
    event_tx: SyncSender<Arc<str>>,
    wake_tx: OwnedFd,
    sink_dead_logged: AtomicBool,
}

impl EventBus {
    /// Bind the events socket, spawn the sink thread, and return the
    /// shareable publish handle together with an [`EventBusHandle`] that
    /// owns the sink's lifetime. The sink owns the socket-file guard, so
    /// the socket file is unlinked exactly when the sink exits.
    pub fn start(socket_path: PathBuf) -> anyhow::Result<(Arc<Self>, EventBusHandle)> {
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)
                .with_context(|| format!("removing stale events socket {:?}", socket_path))?;
        }
        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("binding events socket {:?}", socket_path))?;
        listener.set_nonblocking(true).context("setting events listener non-blocking")?;
        info!("events socket listening at {:?}", socket_path);

        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_CHANNEL_CAP);
        let (wake_rx, wake_tx) = make_self_pipe().context("creating events wake pipe")?;
        let (shutdown_rx, shutdown_tx) =
            make_self_pipe().context("creating events shutdown pipe")?;

        let bus = Arc::new(Self { event_tx, wake_tx, sink_dead_logged: AtomicBool::new(false) });
        let sink = Sink {
            listener,
            event_rx,
            wake_rx,
            shutdown_rx,
            _guard: ListenerGuard { path: socket_path },
        };
        let join = thread::Builder::new()
            .name("events-sink".into())
            .spawn(move || sink.run())
            .context("spawning events sink thread")?;
        Ok((bus, EventBusHandle { shutdown_tx, sink: Some(join) }))
    }

    /// Broadcast `event` to all current subscribers. Non-blocking: a
    /// `try_send` on the publish-to-sink channel + a 1-byte wake. Takes no
    /// internal lock, so it is safe to call under arbitrary outer locks.
    /// Publishing under the lock that protects the state being announced
    /// keeps wire-order = causal-order across mutators.
    pub fn publish(&self, event: &Event) {
        match self.event_tx.try_send(serialize_line(event)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!("events channel full; sink is wedged");
                return;
            }
            Err(TrySendError::Disconnected(_)) => {
                // Relaxed is sufficient: atomic RMW on a single location
                // is linearizable regardless of ordering, so racing swaps
                // already see distinct previous values — exactly one
                // observes `false` and logs. Stronger ordering (e.g.
                // AcqRel) would only matter if we needed happens-before
                // with surrounding non-atomic memory, which we don't.
                if !self.sink_dead_logged.swap(true, Ordering::Relaxed) {
                    error!("events sink died; subsequent events will be dropped");
                }
                return;
            }
        }
        // Wake the sink. A full pipe buffer (EAGAIN) means an unread wake is already
        // pending, so this nudge is redundant -- expected under burst, not logged. Any
        // other errno (EBADF, EIO, ...) is a real fault and is surfaced.
        match unistd::write(&self.wake_tx, b"\0") {
            Ok(_) | Err(Errno::EAGAIN) => {}
            Err(e) => warn!("waking events sink: {e}"),
        }
    }
}

/// Owns the events-sink thread. Dropping it signals the sink to stop --
/// the sink then drops its socket-file guard (unlinking the socket) --
/// and joins the thread, so the sink can never outlive this handle. Not
/// `Clone`: there is exactly one owner of the sink's lifetime, which is
/// what makes "close the bus" a single, deterministic action.
///
/// This is deliberately split from [`EventBus`] rather than joining the sink in
/// `EventBus`'s own `Drop`. `EventBus` is `Arc`-shared across many publishers
/// (the server and the ttl-reaper thread both hold clones), so a `Drop`-based
/// join would fire whenever the *last* clone is released -- a blocking `join()`
/// running from whatever thread happens to drop that clone (plausibly the
/// reaper thread), at a refcount-determined moment in an order not pinned to
/// the rest of daemon teardown. A single non-`Clone` owner instead makes
/// shutdown happen at one known point, thread, and order, and lets the types
/// state what is true: many publishers, one lifetime owner.
pub struct EventBusHandle {
    shutdown_tx: OwnedFd,
    sink: Option<thread::JoinHandle<()>>,
}

impl Drop for EventBusHandle {
    fn drop(&mut self) {
        // Nudge the dedicated shutdown pipe so the sink's `poll` wakes, sees it, and
        // returns (dropping its `ListenerGuard`). `EPIPE` means the sink already exited
        // on its own via the wake-EOF fallback and closed its read end -- expected and
        // benign; the join below is then a no-op. Any other errno is a real fault worth
        // surfacing: a genuinely lost nudge can leave the join below hanging forever.
        match unistd::write(&self.shutdown_tx, b"\0") {
            Ok(_) | Err(Errno::EPIPE) => {}
            Err(e) => warn!("signaling events sink shutdown: {e}"),
        }
        if let Some(join) = self.sink.take() {
            if let Err(e) = join.join() {
                warn!("joining events sink thread: {:?}", e);
            }
        }
    }
}

/// Sibling events socket path next to the main shpool socket. The daemon
/// owns this convention (it binds the socket); the `events` subcommand
/// follows it to connect.
pub fn socket_path(main_socket: &Path) -> PathBuf {
    let mut path = main_socket.to_path_buf();
    path.set_file_name("events.socket");
    path
}

/// Owns the events socket file. Dropping the guard unlinks the socket
/// path so a fresh daemon doesn't trip on stale files. The sink thread is
/// not stopped — daemon shutdown takes the process down.
pub struct ListenerGuard {
    path: PathBuf,
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => warn!("removing events socket {:?}: {:?}", self.path, e),
        }
    }
}

/// All sink-thread-owned state: the listener, the channel/pipe receive
/// ends, and the socket-file guard. The guard is a field so it drops --
/// unlinking the socket -- exactly when [`Sink::run`] returns, so the
/// socket file never outlives the thread serving it.
struct Sink {
    listener: UnixListener,
    event_rx: Receiver<Arc<str>>,
    wake_rx: OwnedFd,
    shutdown_rx: OwnedFd,
    _guard: ListenerGuard,
}

impl Sink {
    fn run(self) {
        let Sink { listener, event_rx, wake_rx, shutdown_rx, _guard } = self;
        let mut subs: Vec<SubscriberWriter> = Vec::new();
        // 4 KiB drain buffer; the bytes are signal-only and discarded.
        let mut wake_buf = [0u8; 4096];
        // Reused across iterations to avoid reallocating each loop. `fds`
        // can't be hoisted: its `PollFd<'_>` element type borrows from
        // `subs[i]`, and the borrow checker tracks that by type, so the
        // borrow on `subs` would persist past `clear()`.
        let mut sub_pollfd_idx: Vec<usize> = Vec::new();
        let mut sub_revents: Vec<PollFlags> = Vec::new();

        // Fixed positions in the poll set: 0 = wake fd, 1 = listener fd
        // (revents ignored -- see below), 2 = shutdown fd, 3.. =
        // subscribers wanting POLLOUT.
        const WAKE_FD_IDX: usize = 0;
        const SHUTDOWN_FD_IDX: usize = 2;
        const SUB_FDS_START: usize = 3;

        loop {
            sub_pollfd_idx.clear();
            sub_revents.clear();

            // Build the poll set fresh each iteration: wake fd (POLLIN),
            // listener fd (POLLIN), each subscriber that wants POLLOUT.
            let mut fds: Vec<PollFd> = Vec::with_capacity(SUB_FDS_START + subs.len());
            fds.push(PollFd::new(wake_rx.as_fd(), PollFlags::POLLIN));
            fds.push(PollFd::new(listener.as_fd(), PollFlags::POLLIN));
            fds.push(PollFd::new(shutdown_rx.as_fd(), PollFlags::POLLIN));
            for (i, sub) in subs.iter().enumerate() {
                if sub.wants_pollout() {
                    fds.push(PollFd::new(sub.as_fd(), PollFlags::POLLOUT));
                    sub_pollfd_idx.push(i);
                }
            }

            match poll::poll(&mut fds, PollTimeout::NONE) {
                Ok(_) => {}
                Err(Errno::EINTR) => continue,
                Err(e) => panic!("events sink poll: {:?}", e),
            }

            // Extract revents into owned values before any mutation: each
            // `PollFd<'fd>` borrows from the fd source (including `subs`), so
            // we drop `fds` before accept / broadcast / drive can run. We
            // ignore the listener fd's revents deliberately; see the
            // always-accept comment below.
            let wake_revents = fds[WAKE_FD_IDX].revents().unwrap_or(PollFlags::empty());
            let shutdown_revents = fds[SHUTDOWN_FD_IDX].revents().unwrap_or(PollFlags::empty());
            sub_revents.extend(
                (0..sub_pollfd_idx.len())
                    .map(|k| fds[SUB_FDS_START + k].revents().unwrap_or(PollFlags::empty())),
            );
            drop(fds);

            // The owning `EventBusHandle` was dropped (it nudged or closed the
            // shutdown pipe). Stop now; returning drops the destructured
            // `_guard`, which unlinks the socket file -- so the socket never
            // outlives the thread serving it.
            if !shutdown_revents.is_empty() {
                return;
            }

            // Always drain pending accepts before processing wake/broadcast.
            // The listener is in the poll set so its POLLIN wakes us, but we
            // don't trust the revent for *whether* to accept: under load,
            // listener POLLIN can lag behind `connect(2)` returning, and if
            // we were woken by the wake-fd alone we still want to catch any
            // queued connections so the same iteration's broadcast reaches
            // them. The accept syscall is cheap (returns WouldBlock
            // immediately when the queue is empty).
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => match SubscriberWriter::new(stream) {
                        Ok(sub) => subs.push(sub),
                        Err(e) => warn!("registering events subscriber: {:?}", e),
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        error!("events listener accept: {:?}", e);
                        break;
                    }
                }
            }

            // POLLHUP on wake_rx fires when the bus is dropped (write end
            // closes); falling through without entering the drain branch
            // would leave POLLHUP latched and `poll()` returning immediately
            // forever. Treat any wake-fd revent as "go read it" -- read will
            // return Ok(0) on EOF and we exit cleanly via that path.
            if !wake_revents.is_empty() {
                // Drain the wake pipe. Ok(0) means the EventBus was dropped;
                // no more events will ever arrive — exit cleanly.
                loop {
                    match unistd::read(&wake_rx, &mut wake_buf) {
                        Ok(0) => return,
                        Ok(_) => continue,
                        Err(Errno::EAGAIN) => break,
                        Err(Errno::EINTR) => continue,
                        Err(e) => panic!("events sink reading wake fd: {:?}", e),
                    }
                }
                // Drain event channel and broadcast. After each enqueue, if
                // the sub's pending was empty before, drive opportunistically:
                // a fast consumer's kernel buffer is likely ready, so the
                // event flushes immediately and the next enqueue starts from
                // empty. Without this, a burst larger than SUBSCRIBER_QUEUE_DEPTH
                // would overflow even healthy subs because broadcast enqueues
                // every event before any drive runs.
                while let Ok(line) = event_rx.try_recv() {
                    for sub in subs.iter_mut() {
                        if sub.dropped {
                            continue;
                        }
                        let was_empty = !sub.wants_pollout();
                        if let Err(Overflow::CapExceeded) = sub.enqueue(Arc::clone(&line)) {
                            warn!("dropping events subscriber: queue full");
                            sub.dropped = true;
                            continue;
                        }
                        if was_empty {
                            if let Err(e) = sub.drive() {
                                info!("events subscriber gone: {:?}", e);
                                sub.dropped = true;
                            }
                        }
                    }
                }
            }

            // Drive writes for subs whose POLLOUT (or error) fired.
            for (k, &i) in sub_pollfd_idx.iter().enumerate() {
                // Short-circuit: a sub marked dropped during the broadcast
                // above (overflow or opportunistic-drive failure) is removed
                // by `subs.retain` at the end of this iteration anyway, but
                // driving it again here would cost a needless `write(2)` on
                // a likely-broken stream.
                if subs[i].dropped {
                    continue;
                }
                let revents = sub_revents[k];
                if revents.intersects(PollFlags::POLLERR | PollFlags::POLLHUP | PollFlags::POLLNVAL)
                {
                    info!("events subscriber gone: peer error/hangup");
                    subs[i].dropped = true;
                } else if revents.contains(PollFlags::POLLOUT) {
                    if let Err(e) = subs[i].drive() {
                        info!("events subscriber gone: {:?}", e);
                        subs[i].dropped = true;
                    }
                }
            }

            subs.retain(|sub| !sub.dropped);
        }
    }
}

/// Per-subscriber state owned by the sink. Exposes an event-shaped surface
/// (`enqueue`, `drive`, `wants_pollout`); the partial-write state machine
/// (offset into `pending.front()`) is hidden from the sink loop.
struct SubscriberWriter {
    stream: UnixStream,
    pending: VecDeque<Arc<str>>,
    front_offset: usize,
    /// Set when the sink decides this sub should be removed; the actual
    /// `Vec` removal happens at end-of-iteration via `subs.retain`. We
    /// can't remove mid-iteration because `sub_pollfd_idx` holds indices
    /// into `subs` and would shift.
    dropped: bool,
}

#[derive(Debug)]
enum Overflow {
    CapExceeded,
}

#[derive(Debug, Eq, PartialEq)]
enum DriveOutcome {
    AllFlushed,
    WouldBlock,
}

impl SubscriberWriter {
    fn new(stream: UnixStream) -> anyhow::Result<Self> {
        stream.set_nonblocking(true).context("setting events subscriber stream non-blocking")?;
        Ok(Self { stream, pending: VecDeque::new(), front_offset: 0, dropped: false })
    }

    fn enqueue(&mut self, line: Arc<str>) -> Result<(), Overflow> {
        if self.pending.len() >= SUBSCRIBER_QUEUE_DEPTH {
            return Err(Overflow::CapExceeded);
        }
        self.pending.push_back(line);
        Ok(())
    }

    fn drive(&mut self) -> io::Result<DriveOutcome> {
        drive_pending(&mut self.stream, &mut self.pending, &mut self.front_offset)
    }

    fn wants_pollout(&self) -> bool {
        !self.pending.is_empty()
    }
}

impl AsFd for SubscriberWriter {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

/// Drains `pending` into `stream` via non-blocking writes, advancing
/// `front_offset` for short writes. Generic over `Write` so the partial-
/// write state machine is unit-testable in isolation against a fake stream.
fn drive_pending<W: Write>(
    stream: &mut W,
    pending: &mut VecDeque<Arc<str>>,
    front_offset: &mut usize,
) -> io::Result<DriveOutcome> {
    while let Some(front) = pending.front() {
        let bytes = &front.as_bytes()[*front_offset..];
        match stream.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "events subscriber stream returned 0 bytes",
                ));
            }
            Ok(n) => {
                *front_offset += n;
                if *front_offset >= front.len() {
                    pending.pop_front();
                    *front_offset = 0;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                return Ok(DriveOutcome::WouldBlock);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(DriveOutcome::AllFlushed)
}

/// Wire-form `Arc<str>` per `Event` variant, lazily built once via serde
/// (so the wire format stays driven by the `#[serde(rename = ...)]`
/// annotations) and cloned cheaply on every publish thereafter. Each
/// `Arc::clone` on the hot path is one atomic increment — no allocation,
/// no serialization.
fn serialize_line(event: &Event) -> Arc<str> {
    fn build(e: Event) -> Arc<str> {
        let s = serde_json::to_string(&e).expect("Event variants are infallible to serialize");
        Arc::from(format!("{s}\n"))
    }
    static CREATED: LazyLock<Arc<str>> = LazyLock::new(|| build(Event::SessionCreated));
    static ATTACHED: LazyLock<Arc<str>> = LazyLock::new(|| build(Event::SessionAttached));
    static DETACHED: LazyLock<Arc<str>> = LazyLock::new(|| build(Event::SessionDetached));
    static REMOVED: LazyLock<Arc<str>> = LazyLock::new(|| build(Event::SessionRemoved));
    match event {
        Event::SessionCreated => Arc::clone(&CREATED),
        Event::SessionAttached => Arc::clone(&ATTACHED),
        Event::SessionDetached => Arc::clone(&DETACHED),
        Event::SessionRemoved => Arc::clone(&REMOVED),
    }
}

fn make_self_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    // A socketpair, not pipe2(2): pipe2 is Linux/BSD-only and absent on
    // macOS. UnixStream::pair() is portable and std sets CLOEXEC on the
    // fds for us so forked children (shells) can't leak them and hold the
    // pipe open past the daemon exiting -- atomically via SOCK_CLOEXEC on
    // Linux, and via an fcntl() fallback on macOS, which has no atomic
    // CLOEXEC primitive for socketpair or pipe. That fallback's fork race
    // is inherent to macOS and identical for any pipe-based design there;
    // we don't make it worse, and keeping the fallback in std beats
    // hand-rolling pipe()+fcntl ourselves. Both ends are then marked
    // non-blocking so the publisher's wake-byte write never stalls and the
    // sink detects "drained" via EAGAIN on read instead of blocking. Only
    // one direction is used (publisher writes, sink reads); the extra
    // socket buffer is immaterial for single-byte wake nudges.
    let (rx, tx) = UnixStream::pair()?;
    rx.set_nonblocking(true)?;
    tx.set_nonblocking(true)?;
    Ok((OwnedFd::from(rx), OwnedFd::from(tx)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::{
        io::{BufRead, BufReader, Read},
        time::{Duration, Instant},
    };

    fn json(event: &Event) -> String {
        serde_json::to_string(event).unwrap()
    }

    /// Per-test scaffolding: tempdir + socket path + bus + sink handle.
    /// `_handle` is declared first so it drops first at end-of-scope --
    /// shutting down and joining the sink (which unlinks the socket)
    /// before the tempdir is removed.
    struct Harness {
        _handle: EventBusHandle,
        bus: Arc<EventBus>,
        path: PathBuf,
        _dir: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.socket");
        let (bus, _handle) = EventBus::start(path.clone()).unwrap();
        Harness { _handle, bus, path, _dir: dir }
    }

    /// Read timeout used for all blocking reads in tests. Generous so
    /// loaded CI machines don't trip the assertion before the sink has a
    /// chance to broadcast.
    const READ_TIMEOUT: Duration = Duration::from_secs(10);

    /// Wire form of `Event::SessionCreated` — the most-published event in
    /// the test suite.
    const CREATED_LINE: &str = "{\"type\":\"session.created\"}\n";

    fn read_line(stream: &mut UnixStream) -> String {
        stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        line
    }

    fn read_n_lines(stream: &mut UnixStream, n: usize) -> Vec<String> {
        stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();
        let mut reader = BufReader::new(stream);
        (0..n)
            .map(|_| {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                line
            })
            .collect()
    }

    /// Connect a fresh subscriber, then publish a probe event and consume
    /// it. Returning means the sink has accepted the connection and
    /// broadcast at least one event to it. Probe is `SessionCreated`;
    /// callers continue with their own publishes from a clean stream.
    fn connect_registered(path: &Path, bus: &EventBus) -> UnixStream {
        let mut stream = UnixStream::connect(path).unwrap();
        // Sleep briefly so the OS schedules the sink thread to accept
        // this connection before the publisher's wake byte arrives.
        // `thread::yield_now()` is only a scheduler hint; under heavy
        // parallel-test load it isn't reliable. A 1ms sleep guarantees
        // a context switch.
        thread::sleep(Duration::from_millis(1));
        bus.publish(&Event::SessionCreated);
        let _ = read_line(&mut stream);
        stream
    }

    /// Like `connect_registered` but for `n` subscribers in one round-trip:
    /// connect them all (queued in the listener backlog), publish one
    /// probe, read it from each. The sink's accept-before-wake order
    /// guarantees all queued subs are registered before the probe is
    /// broadcast.
    fn connect_n_registered(path: &Path, bus: &EventBus, n: usize) -> Vec<UnixStream> {
        let mut streams: Vec<UnixStream> =
            (0..n).map(|_| UnixStream::connect(path).unwrap()).collect();
        thread::sleep(Duration::from_millis(1));
        bus.publish(&Event::SessionCreated);
        for s in streams.iter_mut() {
            let _ = read_line(s);
        }
        streams
    }

    #[test]
    fn events_serialize_with_only_type() {
        let cases = [
            (Event::SessionCreated, r#"{"type":"session.created"}"#),
            (Event::SessionAttached, r#"{"type":"session.attached"}"#),
            (Event::SessionDetached, r#"{"type":"session.detached"}"#),
            (Event::SessionRemoved, r#"{"type":"session.removed"}"#),
        ];
        for (event, expected) in &cases {
            assert_eq!(json(event), *expected, "variant {event:?}");
        }
    }

    #[test]
    fn bus_publish_with_no_subscribers_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let (bus, _handle) = EventBus::start(dir.path().join("events.socket")).unwrap();
        bus.publish(&Event::SessionCreated);
    }

    #[test]
    fn bus_publish_reaches_subscriber() {
        let h = harness();
        let mut stream = connect_registered(&h.path, &h.bus);
        h.bus.publish(&Event::SessionCreated);
        assert_eq!(read_line(&mut stream), CREATED_LINE);
    }

    #[test]
    fn bus_drops_subscriber_whose_peer_closed() {
        let h = harness();
        let victim = connect_registered(&h.path, &h.bus);
        let mut probe = connect_registered(&h.path, &h.bus);

        drop(victim);
        h.bus.publish(&Event::SessionAttached);
        assert_eq!(read_line(&mut probe), "{\"type\":\"session.attached\"}\n");
    }

    // Publish is `try_send` + 1-byte wake -- independent of N. A regression that
    // re-introduced per-sub work in publish would make these timings explode. The
    // absolute threshold is timing-sensitive and can flake on a slow or contended
    // CI runner, so this is kept as executable documentation behind `#[ignore]`
    // rather than a CI gate; run it on demand with `--ignored` if a publish-path
    // regression is ever suspected.
    #[test]
    #[ignore = "timing-sensitive; run on demand with --ignored"]
    fn bus_publish_with_many_subscribers_is_not_quadratic() {
        let h = harness();
        let n = 200;
        let _streams = connect_n_registered(&h.path, &h.bus, n);

        let start = Instant::now();
        for _ in 0..1000 {
            h.bus.publish(&Event::SessionCreated);
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(100),
            "1000 publishes with {n} subs took {elapsed:?}"
        );
    }

    #[test]
    fn bus_concurrent_publish_under_outer_lock_delivers_all_events() {
        // Publish takes no internal lock, so an outer lock can't deadlock
        // with anything inside the bus.
        let h = harness();
        let mut stream = connect_registered(&h.path, &h.bus);

        let outer: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
        let n_threads = 4;
        let n_per_thread = 8;
        let total = n_threads * n_per_thread;

        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let bus = Arc::clone(&h.bus);
                let outer = Arc::clone(&outer);
                thread::spawn(move || {
                    for _ in 0..n_per_thread {
                        let _g = outer.lock();
                        bus.publish(&Event::SessionCreated);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        for line in read_n_lines(&mut stream, total) {
            assert_eq!(line, CREATED_LINE);
        }
    }

    // Dropping the handle must stop the sink AND unlink the socket: the
    // handle joins the sink, the sink owns the socket-file guard, so once
    // `drop` returns the thread is gone and the socket file with it. This
    // pins "the sink can never outlive the socket file."
    #[test]
    fn dropping_handle_stops_sink_and_unlinks_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.socket");
        let (_bus, handle) = EventBus::start(path.clone()).unwrap();
        assert!(path.exists(), "socket file should exist while the sink runs");
        drop(handle);
        assert!(
            !path.exists(),
            "socket file should be unlinked once the sink is shut down and joined"
        );
    }

    #[test]
    fn accept_loop_registers_concurrent_subscribers() {
        let h = harness();
        let n = 20;
        let mut streams: Vec<UnixStream> = (0..n)
            .map(|_| {
                let path = h.path.clone();
                thread::spawn(move || UnixStream::connect(&path).unwrap())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|jh| jh.join().unwrap())
            .collect();

        // Probe to verify each concurrently-dialed sub is registered: the
        // sink's accept-before-wake order ensures all queued connects
        // join `subs` before this publish broadcasts.
        h.bus.publish(&Event::SessionCreated);
        for stream in streams.iter_mut() {
            assert_eq!(read_line(stream), CREATED_LINE);
        }
    }

    #[test]
    fn burst_load_within_capacity_reaches_every_subscriber() {
        let h = harness();
        let m = 4;
        let mut streams = connect_n_registered(&h.path, &h.bus, m);

        // Stay under SUBSCRIBER_QUEUE_DEPTH with margin so no sub is dropped.
        let n_events = 32;
        for _ in 0..n_events {
            h.bus.publish(&Event::SessionCreated);
        }
        let expected = CREATED_LINE;
        for stream in streams.iter_mut() {
            for line in read_n_lines(stream, n_events) {
                assert_eq!(line, expected);
            }
        }
    }

    #[test]
    fn events_arrive_in_publish_order() {
        let h = harness();
        let mut stream = connect_registered(&h.path, &h.bus);

        h.bus.publish(&Event::SessionCreated);
        h.bus.publish(&Event::SessionAttached);
        h.bus.publish(&Event::SessionDetached);
        h.bus.publish(&Event::SessionRemoved);

        let lines = read_n_lines(&mut stream, 4);
        assert_eq!(
            lines,
            [
                CREATED_LINE,
                "{\"type\":\"session.attached\"}\n",
                "{\"type\":\"session.detached\"}\n",
                "{\"type\":\"session.removed\"}\n",
            ]
        );
    }

    #[test]
    fn slow_subscriber_drop_does_not_affect_fast_through_sink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.socket");
        let (bus, _handle) = EventBus::start(path.clone()).unwrap();

        // Small SO_RCVBUF on slow caps how much un-read data the kernel
        // will buffer en-route to it; the sink's writes start returning
        // EAGAIN, slow's pending grows to cap, slow is dropped.
        let _slow = UnixStream::connect(&path).unwrap();
        nix::sys::socket::setsockopt(&_slow, nix::sys::socket::sockopt::RcvBuf, &1024).unwrap();
        let mut fast = UnixStream::connect(&path).unwrap();

        // Probe both subs (fast reads it; slow's tiny buffer fits one
        // event of ~30 bytes).
        bus.publish(&Event::SessionCreated);
        let _ = read_line(&mut fast);

        // Interleave publish + read so fast's kernel buffer never fills
        // (the default AF_UNIX RcvBuf is small enough that buffering K
        // events without reading would overflow fast's pending and cause
        // the sink to drop fast). Slow gets enqueued every iteration too;
        // its small RcvBuf forces drive into EAGAIN within a few dozen
        // events, then pending overflows and the sink drops slow. The
        // test passes only if fast continues receiving while slow is
        // overflowing or after slow is dropped.
        let expected = CREATED_LINE;
        fast.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut reader = BufReader::new(&mut fast);
        for _ in 0..1000 {
            bus.publish(&Event::SessionCreated);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, expected);
        }
    }

    /// Fake writer: per-call accept counts. Each `write` consumes the next
    /// entry: `Some(n)` accepts `min(n, buf.len())` bytes; `None` (or 0)
    /// returns WouldBlock; `Err` returns the given error.
    struct FakeWriter {
        plan: VecDeque<FakeWrite>,
        written: Vec<u8>,
    }

    enum FakeWrite {
        Accept(usize),
        WouldBlock,
        Interrupted,
        Err(io::ErrorKind),
    }

    impl Write for FakeWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self.plan.pop_front() {
                Some(FakeWrite::Accept(n)) => {
                    let take = n.min(buf.len());
                    self.written.extend_from_slice(&buf[..take]);
                    Ok(take)
                }
                Some(FakeWrite::WouldBlock) | None => {
                    Err(io::Error::new(io::ErrorKind::WouldBlock, "fake EAGAIN"))
                }
                Some(FakeWrite::Interrupted) => {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "fake EINTR"))
                }
                Some(FakeWrite::Err(kind)) => Err(io::Error::new(kind, "fake error")),
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn drive_pending_flushes_a_single_complete_write() {
        let mut w = FakeWriter { plan: VecDeque::from([FakeWrite::Accept(100)]), written: vec![] };
        let mut pending: VecDeque<Arc<str>> = VecDeque::from([Arc::from("hello\n")]);
        let mut offset = 0;
        let outcome = drive_pending(&mut w, &mut pending, &mut offset).unwrap();
        assert_eq!(outcome, DriveOutcome::AllFlushed);
        assert_eq!(w.written, b"hello\n");
        assert!(pending.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn drive_pending_resumes_after_partial_then_wouldblock() {
        // Accept 3, then EAGAIN; resume across two further drives.
        let mut w = FakeWriter {
            plan: VecDeque::from([
                FakeWrite::Accept(3),
                FakeWrite::WouldBlock,
                FakeWrite::Accept(3),
                FakeWrite::WouldBlock,
            ]),
            written: vec![],
        };
        let mut pending: VecDeque<Arc<str>> = VecDeque::from([Arc::from("hello\n")]);
        let mut offset = 0;

        // 1st: writes "hel", then WouldBlock.
        let outcome = drive_pending(&mut w, &mut pending, &mut offset).unwrap();
        assert_eq!(outcome, DriveOutcome::WouldBlock);
        assert_eq!(w.written, b"hel");
        assert_eq!(offset, 3);
        assert_eq!(pending.len(), 1);

        // 2nd: writes "lo\n" (the remainder), then WouldBlock with empty pending.
        let outcome = drive_pending(&mut w, &mut pending, &mut offset).unwrap();
        assert_eq!(outcome, DriveOutcome::AllFlushed);
        assert_eq!(w.written, b"hello\n");
        assert_eq!(offset, 0);
        assert!(pending.is_empty());
    }

    #[test]
    fn drive_pending_retries_on_eintr() {
        let mut w = FakeWriter {
            plan: VecDeque::from([FakeWrite::Interrupted, FakeWrite::Accept(100)]),
            written: vec![],
        };
        let mut pending: VecDeque<Arc<str>> = VecDeque::from([Arc::from("ok\n")]);
        let mut offset = 0;
        let outcome = drive_pending(&mut w, &mut pending, &mut offset).unwrap();
        assert_eq!(outcome, DriveOutcome::AllFlushed);
        assert_eq!(w.written, b"ok\n");
    }

    #[test]
    fn drive_pending_propagates_other_errors() {
        let mut w = FakeWriter {
            plan: VecDeque::from([FakeWrite::Err(io::ErrorKind::BrokenPipe)]),
            written: vec![],
        };
        let mut pending: VecDeque<Arc<str>> = VecDeque::from([Arc::from("x\n")]);
        let mut offset = 0;
        let err = drive_pending(&mut w, &mut pending, &mut offset).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn drive_pending_treats_zero_byte_write_as_error() {
        let mut w = FakeWriter { plan: VecDeque::from([FakeWrite::Accept(0)]), written: vec![] };
        let mut pending: VecDeque<Arc<str>> = VecDeque::from([Arc::from("x\n")]);
        let mut offset = 0;
        // Note: FakeWrite::Accept(0) returns Ok(0) (not WouldBlock) because
        // we haven't written WouldBlock to the plan; this is a write-zero
        // signal which `drive_pending` must treat as an error.
        let err = drive_pending(&mut w, &mut pending, &mut offset).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WriteZero);
    }

    /// Scripted-scenario sweep: hand-rolled sequences of partial /
    /// EAGAIN / EINTR / full-accept responses, paired with new enqueues
    /// interleaved between drives, must eventually flush every enqueued
    /// byte once the plan ends with enough accepts to drain.
    #[test]
    fn drive_pending_scripted_scenarios_flush_completely() {
        // Hand-rolled deterministic interleavings covering the
        // "queue-state-changed / POLLOUT-requested" atomicity: each row is
        // a sequence of (op, write-plan-entries) where op is "drive once"
        // or "enqueue X". After running the whole script, drive to
        // completion with a generous accept-all and assert every enqueued
        // byte was written, in order.
        enum Op {
            Enqueue(&'static str),
            Drive(Vec<FakeWrite>),
        }
        use FakeWrite::*;

        let scripts: Vec<Vec<Op>> = vec![
            vec![Op::Enqueue("a\n"), Op::Drive(vec![Accept(1), WouldBlock])],
            vec![
                Op::Enqueue("a\n"),
                Op::Drive(vec![Accept(1)]),
                Op::Enqueue("b\n"),
                Op::Drive(vec![Accept(1), Interrupted, Accept(2), Accept(2)]),
            ],
            vec![
                Op::Enqueue("hello\n"),
                Op::Drive(vec![WouldBlock]),
                Op::Enqueue("world\n"),
                Op::Drive(vec![Accept(2), Interrupted, WouldBlock]),
                Op::Enqueue("again\n"),
            ],
            vec![
                Op::Enqueue("x\n"),
                Op::Enqueue("y\n"),
                Op::Enqueue("z\n"),
                Op::Drive(vec![Accept(1), Accept(1), WouldBlock]),
                Op::Drive(vec![Accept(4), Accept(2)]),
            ],
        ];

        for (i, script) in scripts.into_iter().enumerate() {
            let mut pending: VecDeque<Arc<str>> = VecDeque::new();
            let mut offset = 0;
            let mut all_written = Vec::new();
            let mut expected = Vec::new();

            for op in script {
                match op {
                    Op::Enqueue(s) => {
                        pending.push_back(Arc::from(s));
                        expected.extend_from_slice(s.as_bytes());
                    }
                    Op::Drive(plan) => {
                        let mut w = FakeWriter { plan: plan.into(), written: vec![] };
                        let _ = drive_pending(&mut w, &mut pending, &mut offset);
                        all_written.extend_from_slice(&w.written);
                    }
                }
            }

            // Final drive with unlimited accept across many calls;
            // drive_pending invokes `write` once per pending entry.
            let mut w = FakeWriter {
                plan: std::iter::repeat_with(|| FakeWrite::Accept(usize::MAX)).take(64).collect(),
                written: vec![],
            };
            let outcome = drive_pending(&mut w, &mut pending, &mut offset).unwrap();
            assert_eq!(outcome, DriveOutcome::AllFlushed, "script {i}");
            all_written.extend_from_slice(&w.written);

            assert_eq!(all_written, expected, "script {i}: bytes lost or reordered");
            assert!(pending.is_empty(), "script {i}: pending not drained");
            assert_eq!(offset, 0, "script {i}: offset not reset");
        }
    }

    #[test]
    fn enqueue_overflows_at_cap() {
        let (a, _b) = UnixStream::pair().unwrap();
        let mut sub = SubscriberWriter::new(a).unwrap();
        for i in 0..SUBSCRIBER_QUEUE_DEPTH {
            sub.enqueue(format!("event-{i}\n").into()).unwrap();
        }
        let err = sub.enqueue("one-too-many\n".into());
        assert!(matches!(err, Err(Overflow::CapExceeded)));
    }

    #[test]
    fn subscriber_writer_overflows_when_peer_blocks() {
        // Use a socket pair so we can shrink the *server-side* send buffer
        // (a listener-accepted socket isn't reachable from outside the
        // sink). With the peer never reading, drive() eventually returns
        // WouldBlock; pending grows past the cap and enqueue fails.
        let (server, _client) = UnixStream::pair().unwrap();
        nix::sys::socket::setsockopt(&server, nix::sys::socket::sockopt::SndBuf, &1024).unwrap();
        let mut sub = SubscriberWriter::new(server).unwrap();

        let line: Arc<str> = CREATED_LINE.into();
        let mut overflowed = false;
        for _ in 0..(SUBSCRIBER_QUEUE_DEPTH * 1000) {
            let was_empty = !sub.wants_pollout();
            match sub.enqueue(Arc::clone(&line)) {
                Ok(()) => {
                    if was_empty {
                        let _ = sub.drive();
                    }
                }
                Err(Overflow::CapExceeded) => {
                    overflowed = true;
                    break;
                }
            }
        }
        assert!(overflowed, "expected SubscriberWriter to overflow");
    }

    #[test]
    fn subscriber_writer_resumes_after_peer_drains() {
        // After a SubscriberWriter has piled up pending against a full
        // kernel buffer, the next `drive()` must make progress once the
        // peer drains. Pairs the fake-stream resume property
        // (`drive_pending_resumes_after_partial_then_wouldblock`) with a
        // real socket.
        let (server, client) = UnixStream::pair().unwrap();
        nix::sys::socket::setsockopt(&server, nix::sys::socket::sockopt::SndBuf, &1024).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut sub = SubscriberWriter::new(server).unwrap();

        let line: Arc<str> = CREATED_LINE.into();
        for _ in 0..SUBSCRIBER_QUEUE_DEPTH {
            sub.enqueue(Arc::clone(&line)).unwrap();
        }

        // First drive: kernel buffer fills, returns WouldBlock with
        // pending non-empty.
        assert_eq!(sub.drive().unwrap(), DriveOutcome::WouldBlock);
        let pending_after_first = sub.pending.len();
        assert!(pending_after_first > 0);

        // Drain whatever the peer can read in one shot.
        let mut buf = vec![0u8; 4096];
        let drained = (&client).read(&mut buf).unwrap();
        assert!(drained > 0);

        // Second drive: must shrink pending (proves resume).
        let _ = sub.drive().unwrap();
        assert!(
            sub.pending.len() < pending_after_first,
            "drive must advance pending after peer drains: {} -> {}",
            pending_after_first,
            sub.pending.len()
        );
    }

    #[test]
    fn fast_writer_unaffected_when_slow_overflows() {
        // Slow: small SndBuf, peer never reads --> drive blocks --> pending
        // grows to cap --> enqueue fails.
        // Fast: default SndBuf, peer drains --> drive flushes --> pending
        // stays empty --> enqueue never fails.
        let (slow_server, _slow_client) = UnixStream::pair().unwrap();
        nix::sys::socket::setsockopt(&slow_server, nix::sys::socket::sockopt::SndBuf, &1024)
            .unwrap();
        let mut slow = SubscriberWriter::new(slow_server).unwrap();

        let (fast_server, fast_client) = UnixStream::pair().unwrap();
        let mut fast = SubscriberWriter::new(fast_server).unwrap();

        let drainer = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut total = 0usize;
            loop {
                match (&fast_client).read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => total += n,
                    Err(_) => break,
                }
            }
            total
        });

        let line: Arc<str> = CREATED_LINE.into();
        let mut slow_dropped = false;

        // Mimic the sink's enqueue+opportunistic-drive pattern across both
        // subs for each event.
        for _ in 0..(SUBSCRIBER_QUEUE_DEPTH * 100) {
            if !slow_dropped {
                let was_empty = !slow.wants_pollout();
                match slow.enqueue(Arc::clone(&line)) {
                    Ok(()) => {
                        if was_empty {
                            let _ = slow.drive();
                        }
                    }
                    Err(Overflow::CapExceeded) => {
                        slow_dropped = true;
                    }
                }
            }
            let was_empty = !fast.wants_pollout();
            fast.enqueue(Arc::clone(&line)).expect("fast must not overflow");
            if was_empty {
                let _ = fast.drive();
            }
            if slow_dropped {
                break;
            }
        }

        assert!(slow_dropped, "slow should have overflowed");

        // Closing fast's server end lets the drainer thread exit on EOF.
        drop(fast);
        let bytes_received = drainer.join().unwrap();
        assert!(bytes_received > 0, "fast should have received events");
    }
}
