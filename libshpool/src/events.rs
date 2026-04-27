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

use serde_derive::Serialize;
use shpool_protocol::Session;

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
