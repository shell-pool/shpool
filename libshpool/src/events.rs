//! Push-event protocol for the daemon.
//!
//! Events are published to subscribers connected to a sibling Unix socket
//! next to the main shpool socket. The wire format is JSON, one event per
//! line (newline-delimited; aka JSONL). Non-Rust clients only need a Unix
//! socket and a JSON parser to consume the stream.
//!
//! Events carry no payload beyond their type — they signal that *something*
//! changed in the session table. Subscribers learn the new state by calling
//! `shpool list` (or the equivalent over the main socket). Subscribers that
//! fall too far behind are dropped and may simply reconnect.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::Context;
use serde_derive::Serialize;
use tracing::{error, info, warn};

/// Per-subscriber outbound queue depth. Subscribers that fall this far
/// behind are dropped and must reconnect.
const SUBSCRIBER_QUEUE_DEPTH: usize = 64;

/// Write timeout for stuck subscribers (e.g. suspended via Ctrl-Z). After
/// this elapses on a blocked write, the writer thread exits and the
/// subscriber is implicitly dropped on the next publish.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Fans out events to all connected subscribers.
///
/// Lock ordering: callers that publish under another lock (e.g. the session
/// table) must take that lock before [`EventBus::publish`] takes its own
/// internal lock. Publishing under the table lock keeps wire-order =
/// causal-order across mutators.
pub struct EventBus {
    subscribers: Mutex<Vec<SyncSender<Arc<str>>>>,
}

impl EventBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { subscribers: Mutex::new(Vec::new()) })
    }

    /// Broadcast `event` to all current subscribers. Subscribers whose
    /// queues are full or whose receivers have hung up are dropped.
    pub fn publish(&self, event: &Event) {
        let line = match serialize_line(event) {
            Some(line) => line,
            None => return,
        };
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| match tx.try_send(line.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                warn!("dropping events subscriber: queue full");
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    /// Register a new subscriber. Returns the receiver to be handed to a
    /// writer thread.
    pub fn register(&self) -> Receiver<Arc<str>> {
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_QUEUE_DEPTH);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
}

fn serialize_line(event: &Event) -> Option<Arc<str>> {
    match serde_json::to_string(event) {
        Ok(s) => Some(format!("{s}\n").into()),
        Err(e) => {
            error!("serializing event {:?}: {:?}", event, e);
            None
        }
    }
}

/// Connect to the events socket, copy each line to stdout, and flush per
/// line so the stream is usable in pipes (`shpool events | jq`). Returns
/// when the daemon closes the connection.
pub fn subscribe_to_stdout(socket_path: &Path) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connecting to events socket {:?}", socket_path))?;
    let reader = BufReader::new(stream);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in reader.lines() {
        let line = line.context("reading event")?;
        writeln!(out, "{line}").context("writing event")?;
        out.flush().context("flushing stdout")?;
    }
    Ok(())
}

/// Sibling events socket path next to the main shpool socket.
pub fn socket_path(main_socket: &Path) -> PathBuf {
    let mut path = main_socket.to_path_buf();
    path.set_file_name("events.socket");
    path
}

/// Owns the events socket file. Dropping the guard unlinks the socket
/// path so a fresh daemon doesn't trip on stale files. The accept thread
/// is not stopped — daemon shutdown takes the process down.
pub struct ListenerGuard {
    path: PathBuf,
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("removing events socket {:?}: {:?}", self.path, e),
        }
    }
}

/// Bind the events socket and spawn the accept thread. For each accepted
/// connection, `on_accept` is invoked with the stream; it is expected to
/// register the subscriber with the bus and spawn a writer thread (see
/// [`spawn_writer`]). The returned guard unlinks the socket file on drop.
pub fn start_listener<F>(socket_path: PathBuf, on_accept: F) -> anyhow::Result<ListenerGuard>
where
    F: Fn(UnixStream) -> anyhow::Result<()> + Send + 'static,
{
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("removing stale events socket {:?}", socket_path))?;
    }
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("binding events socket {:?}", socket_path))?;
    info!("events socket listening at {:?}", socket_path);
    thread::Builder::new()
        .name("events-accept".into())
        .spawn(move || run_accept_loop(listener, on_accept))
        .context("spawning events accept thread")?;
    Ok(ListenerGuard { path: socket_path })
}

fn run_accept_loop<F>(listener: UnixListener, on_accept: F)
where
    F: Fn(UnixStream) -> anyhow::Result<()>,
{
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = on_accept(stream) {
                    warn!("accepting events subscriber: {:?}", e);
                }
            }
            Err(e) => {
                error!("events listener accept failed: {:?}", e);
                break;
            }
        }
    }
}

/// Set the write timeout and spawn a thread that drains `receiver` to
/// `stream` until either side closes or a write times out.
pub fn spawn_writer(stream: UnixStream, receiver: Receiver<Arc<str>>) -> anyhow::Result<()> {
    stream.set_write_timeout(Some(WRITE_TIMEOUT)).context("setting write timeout")?;
    thread::Builder::new()
        .name("events-writer".into())
        .spawn(move || run_writer(stream, receiver))
        .context("spawning events writer thread")?;
    Ok(())
}

fn run_writer(mut stream: UnixStream, receiver: Receiver<Arc<str>>) {
    while let Ok(line) = receiver.recv() {
        if let Err(e) = stream.write_all(line.as_bytes()) {
            info!("events subscriber gone: {:?}", e);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(event: &Event) -> String {
        serde_json::to_string(event).unwrap()
    }

    #[test]
    fn session_created_serializes_with_only_type() {
        assert_eq!(json(&Event::SessionCreated), r#"{"type":"session.created"}"#);
    }

    #[test]
    fn session_attached_serializes_with_only_type() {
        assert_eq!(json(&Event::SessionAttached), r#"{"type":"session.attached"}"#);
    }

    #[test]
    fn session_detached_serializes_with_only_type() {
        assert_eq!(json(&Event::SessionDetached), r#"{"type":"session.detached"}"#);
    }

    #[test]
    fn session_removed_serializes_with_only_type() {
        assert_eq!(json(&Event::SessionRemoved), r#"{"type":"session.removed"}"#);
    }

    #[test]
    fn bus_publish_with_no_subscribers_is_a_noop() {
        let bus = EventBus::new();
        bus.publish(&Event::SessionCreated);
    }

    #[test]
    fn bus_publish_reaches_subscriber() {
        let bus = EventBus::new();
        let rx = bus.register();
        bus.publish(&Event::SessionCreated);
        let line = rx.recv().unwrap();
        assert_eq!(&*line, "{\"type\":\"session.created\"}\n");
    }

    #[test]
    fn bus_drops_subscriber_whose_queue_is_full() {
        let bus = EventBus::new();
        let rx = bus.register();
        for _ in 0..SUBSCRIBER_QUEUE_DEPTH {
            bus.publish(&Event::SessionCreated);
        }
        assert_eq!(bus.subscribers.lock().unwrap().len(), 1);
        bus.publish(&Event::SessionCreated);
        assert_eq!(bus.subscribers.lock().unwrap().len(), 0);
        drop(rx);
    }

    #[test]
    fn bus_drops_subscriber_whose_receiver_hung_up() {
        let bus = EventBus::new();
        let rx = bus.register();
        drop(rx);
        bus.publish(&Event::SessionCreated);
        assert_eq!(bus.subscribers.lock().unwrap().len(), 0);
    }

    #[test]
    fn bus_publish_reaches_every_subscriber() {
        let bus = EventBus::new();
        let rx_a = bus.register();
        let rx_b = bus.register();
        bus.publish(&Event::SessionCreated);
        for rx in [&rx_a, &rx_b] {
            let line = rx.recv().unwrap();
            assert_eq!(&*line, "{\"type\":\"session.created\"}\n");
        }
    }

    #[test]
    fn writer_exits_when_peer_closes_stream() {
        let (a, b) = UnixStream::pair().unwrap();
        let (tx, rx) = mpsc::sync_channel::<Arc<str>>(8);
        let handle = thread::spawn(move || run_writer(a, rx));
        drop(b);
        // The send may succeed (kernel buffered) or fail; what matters is
        // that closing the channel unblocks the writer thread on the next
        // recv, regardless of write outcome.
        let _ = tx.try_send("ignored\n".into());
        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn spawn_writer_sets_write_timeout() {
        let (a, _b) = UnixStream::pair().unwrap();
        let probe = a.try_clone().unwrap();
        let (_tx, rx) = mpsc::sync_channel(1);
        spawn_writer(a, rx).unwrap();
        assert_eq!(probe.write_timeout().unwrap(), Some(WRITE_TIMEOUT));
    }

    #[test]
    fn listener_guard_unlinks_socket_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.socket");
        let guard = start_listener(path.clone(), |_| Ok(())).unwrap();
        assert!(path.exists(), "socket file should exist while guard is alive");
        drop(guard);
        assert!(!path.exists(), "socket file should be unlinked on guard drop");
    }
}
