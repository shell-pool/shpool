//! Push-event protocol for the daemon.
//!
//! Events are published to subscribers connected to a sibling Unix socket
//! next to the main shpool socket. The wire format is JSON, one event per
//! line (newline-delimited; aka JSONL). Non-Rust clients only need a Unix
//! socket and a JSON parser to consume the stream.
//!
//! On connect, a subscriber receives a `snapshot` event reflecting the
//! current session table, atomically with respect to the table mutations
//! that produce subsequent delta events. After the snapshot, the subscriber
//! receives delta events as the session table changes. To force a re-sync,
//! a subscriber may simply reconnect.
//!
//! The `sessions` field of a snapshot event uses the same schema as the
//! `sessions` field of `shpool list --json`, so the two surfaces stay in
//! sync by construction.

use std::{
    io::Write,
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
use shpool_protocol::Session;
use tracing::{error, info, warn};

/// Per-subscriber outbound queue depth. Subscribers that fall this far
/// behind are dropped; reconnection re-syncs them via a fresh snapshot.
const SUBSCRIBER_QUEUE_DEPTH: usize = 64;

/// Write timeout for stuck subscribers (e.g. suspended via Ctrl-Z). After
/// this elapses on a blocked write, the writer thread exits and the
/// subscriber is implicitly dropped on the next publish.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// An event published on the events socket.
#[derive(Serialize, Debug)]
#[serde(tag = "type")]
pub enum Event {
    /// Sent as the first message after a subscriber connects, reflecting
    /// the current session table.
    #[serde(rename = "snapshot")]
    Snapshot { sessions: Vec<Session> },

    /// A new session was created.
    #[serde(rename = "session.created")]
    SessionCreated { name: String, started_at_unix_ms: i64 },

    /// A client attached to an existing session.
    #[serde(rename = "session.attached")]
    SessionAttached { name: String, last_connected_at_unix_ms: i64 },

    /// A client detached from a session that is still alive.
    #[serde(rename = "session.detached")]
    SessionDetached { name: String, last_disconnected_at_unix_ms: i64 },

    /// A session was removed from the session table.
    #[serde(rename = "session.removed")]
    SessionRemoved { name: String, reason: RemovedReason },
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RemovedReason {
    /// The shell process exited on its own.
    Exited,
    /// The session was killed by an explicit `shpool kill` request.
    Killed,
}

/// Fans out events to all connected subscribers.
///
/// Lock ordering: callers that publish under another lock (e.g. the session
/// table) must take that lock before [`EventBus::publish`] takes its own
/// internal lock. [`EventBus::register`] follows the same order, so a
/// subscriber registered while the session-table lock is held cannot miss
/// a delta from a mutation that committed under that lock.
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

    /// Register a new subscriber with `snapshot` as the first message in
    /// its queue. Returns the receiver to be handed to a writer thread.
    pub fn register(&self, snapshot: &Event) -> Receiver<Arc<str>> {
        let line = serialize_line(snapshot).expect("snapshot serialization");
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_QUEUE_DEPTH);
        tx.try_send(line).expect("seeding empty channel cannot fail");
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
pub fn start_listener<F>(
    socket_path: PathBuf,
    on_accept: F,
) -> anyhow::Result<ListenerGuard>
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
    use shpool_protocol::SessionStatus;

    fn json(event: &Event) -> String {
        serde_json::to_string(event).unwrap()
    }

    #[test]
    fn snapshot_serializes_with_sessions_array() {
        let event = Event::Snapshot {
            sessions: vec![Session {
                name: "main".into(),
                started_at_unix_ms: 100,
                last_connected_at_unix_ms: Some(200),
                last_disconnected_at_unix_ms: None,
                status: SessionStatus::Attached,
            }],
        };
        assert_eq!(
            json(&event),
            r#"{"type":"snapshot","sessions":[{"name":"main","started_at_unix_ms":100,"last_connected_at_unix_ms":200,"last_disconnected_at_unix_ms":null,"status":"Attached"}]}"#
        );
    }

    #[test]
    fn session_created_serializes_flat() {
        let event = Event::SessionCreated { name: "main".into(), started_at_unix_ms: 42 };
        assert_eq!(json(&event), r#"{"type":"session.created","name":"main","started_at_unix_ms":42}"#);
    }

    #[test]
    fn session_attached_serializes_flat() {
        let event =
            Event::SessionAttached { name: "main".into(), last_connected_at_unix_ms: 42 };
        assert_eq!(
            json(&event),
            r#"{"type":"session.attached","name":"main","last_connected_at_unix_ms":42}"#
        );
    }

    #[test]
    fn session_detached_serializes_flat() {
        let event =
            Event::SessionDetached { name: "main".into(), last_disconnected_at_unix_ms: 42 };
        assert_eq!(
            json(&event),
            r#"{"type":"session.detached","name":"main","last_disconnected_at_unix_ms":42}"#
        );
    }

    #[test]
    fn bus_publish_with_no_subscribers_is_a_noop() {
        let bus = EventBus::new();
        bus.publish(&Event::SessionCreated { name: "x".into(), started_at_unix_ms: 1 });
    }

    #[test]
    fn bus_register_seeds_receiver_with_snapshot() {
        let bus = EventBus::new();
        let snapshot = Event::Snapshot { sessions: vec![] };
        let rx = bus.register(&snapshot);
        let line = rx.try_recv().unwrap();
        assert_eq!(&*line, "{\"type\":\"snapshot\",\"sessions\":[]}\n");
    }

    #[test]
    fn bus_publish_reaches_subscriber_after_snapshot() {
        let bus = EventBus::new();
        let rx = bus.register(&Event::Snapshot { sessions: vec![] });
        bus.publish(&Event::SessionCreated { name: "main".into(), started_at_unix_ms: 7 });
        let snapshot_line = rx.recv().unwrap();
        let delta_line = rx.recv().unwrap();
        assert_eq!(&*snapshot_line, "{\"type\":\"snapshot\",\"sessions\":[]}\n");
        assert_eq!(
            &*delta_line,
            "{\"type\":\"session.created\",\"name\":\"main\",\"started_at_unix_ms\":7}\n"
        );
    }

    #[test]
    fn bus_drops_subscriber_whose_queue_is_full() {
        let bus = EventBus::new();
        let rx = bus.register(&Event::Snapshot { sessions: vec![] });
        // Fill the channel to capacity (the snapshot already used 1 slot).
        for i in 0..(SUBSCRIBER_QUEUE_DEPTH - 1) {
            bus.publish(&Event::SessionCreated {
                name: format!("s{i}"),
                started_at_unix_ms: i as i64,
            });
        }
        assert_eq!(bus.subscribers.lock().unwrap().len(), 1);
        // One more publish overflows and the subscriber is dropped.
        bus.publish(&Event::SessionCreated { name: "overflow".into(), started_at_unix_ms: 0 });
        assert_eq!(bus.subscribers.lock().unwrap().len(), 0);
        // The receiver still has the buffered events; the channel is not
        // closed for it from the receiving side.
        drop(rx);
    }

    #[test]
    fn bus_drops_subscriber_whose_receiver_hung_up() {
        let bus = EventBus::new();
        let rx = bus.register(&Event::Snapshot { sessions: vec![] });
        drop(rx);
        bus.publish(&Event::SessionCreated { name: "x".into(), started_at_unix_ms: 0 });
        assert_eq!(bus.subscribers.lock().unwrap().len(), 0);
    }

    #[test]
    fn bus_publish_reaches_every_subscriber() {
        let bus = EventBus::new();
        let rx_a = bus.register(&Event::Snapshot { sessions: vec![] });
        let rx_b = bus.register(&Event::Snapshot { sessions: vec![] });
        bus.publish(&Event::SessionCreated { name: "main".into(), started_at_unix_ms: 1 });
        for rx in [&rx_a, &rx_b] {
            let _snapshot = rx.recv().unwrap();
            let delta = rx.recv().unwrap();
            assert!(delta.contains(r#""type":"session.created""#));
            assert!(delta.contains(r#""name":"main""#));
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

    #[test]
    fn session_removed_serializes_with_reason() {
        let exited =
            Event::SessionRemoved { name: "main".into(), reason: RemovedReason::Exited };
        assert_eq!(
            json(&exited),
            r#"{"type":"session.removed","name":"main","reason":"exited"}"#
        );
        let killed =
            Event::SessionRemoved { name: "main".into(), reason: RemovedReason::Killed };
        assert_eq!(
            json(&killed),
            r#"{"type":"session.removed","name":"main","reason":"killed"}"#
        );
    }
}
