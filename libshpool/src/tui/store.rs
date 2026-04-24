//! `SessionStore` — a thin wrapper around the shpool protocol Client
//! that the executor uses to talk to the daemon.
//!
//! We use the protocol directly (not shell-out to `shpool list`/
//! `shpool kill`). Attach is NOT here — attach stays behind a
//! subprocess boundary so it can take over the terminal the way a
//! normal `shpool attach` does. See `attach.rs`.

use std::{cell::RefCell, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use shpool_protocol::{ConnectHeader, KillReply, KillRequest, ListReply, Session};

use crate::protocol;

/// Owns the socket path and makes protocol requests on demand.
///
/// We don't hold a long-lived connection — each request opens a fresh
/// one. The daemon's protocol is request/response and the Client
/// isn't designed for reuse, so one-shot is both simplest and
/// consistent with how the existing `list`/`kill` subcommands work.
///
/// Combined with the update layer's auto-refresh (one `list` per
/// Normal-mode keystroke), this amounts to a connect + write + read
/// round-trip per keypress. Sub-millisecond on a local unix socket;
/// not perceivable.
pub struct SessionStore {
    /// Path to the daemon's unix socket. Stored by value because we
    /// hand it out by reference to spawn_attach too.
    pub socket: PathBuf,

    /// First version-mismatch warning seen from any `connect`. The
    /// TUI displays this once in the footer (via `take_first_warning`
    /// from `mod.rs`) so the user learns about protocol drift without
    /// the warning stuttering on every subsequent refresh.
    ///
    /// `RefCell` gives us interior mutability: `list` and `kill`
    /// take `&self`, and we want to update the warning slot as a
    /// side effect of a successful connect.
    first_warning: RefCell<Option<String>>,
}

impl SessionStore {
    pub fn new(socket: PathBuf) -> Self {
        Self { socket, first_warning: RefCell::new(None) }
    }

    /// Consume and return the first version-mismatch warning we saw,
    /// if any. Returns None thereafter — intended to be called once
    /// at startup to surface the warning in the footer.
    pub fn take_first_warning(&self) -> Option<String> {
        self.first_warning.borrow_mut().take()
    }

    /// Fetch the session list.
    pub fn list(&self) -> Result<Vec<Session>> {
        let mut client = self.connect()?;
        client.write_connect_header(ConnectHeader::List).context("sending list connect header")?;
        let reply: ListReply = client.read_reply().context("reading list reply")?;
        Ok(reply.sessions)
    }

    /// Kill one session by name. Returns Ok(()) on success; Err with a
    /// display-ready message otherwise.
    pub fn kill(&self, name: &str) -> Result<()> {
        let mut client = self.connect()?;
        client
            .write_connect_header(ConnectHeader::Kill(KillRequest {
                sessions: vec![name.to_string()],
            }))
            .context("sending kill connect header")?;
        let reply: KillReply = client.read_reply().context("reading kill reply")?;
        // Single-session caller: `not_found_sessions` has at most one
        // entry. The loop-like `join(" ")` is kept for symmetry with
        // the CLI `kill` path rather than because multiple names are
        // possible here.
        if !reply.not_found_sessions.is_empty() {
            return Err(anyhow!("not found: {}", reply.not_found_sessions.join(" ")));
        }
        Ok(())
    }

    /// Open a new protocol client. Uses the shared [`crate::protocol::connect`]
    /// helper so the connect-dance (version-mismatch handling,
    /// IO-error downcast) lives in one place. Any version-mismatch
    /// warning is captured into `first_warning` on the *first*
    /// occurrence and then ignored; see `take_first_warning`.
    fn connect(&self) -> Result<protocol::Client> {
        let (client, warning) = protocol::connect(&self.socket)?;
        if let Some(w) = warning {
            let mut slot = self.first_warning.borrow_mut();
            if slot.is_none() {
                *slot = Some(w);
            }
        }
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_first_warning_is_idempotent() {
        let store = SessionStore::new(PathBuf::from("/nonexistent"));
        // Seed a warning directly (simulating a successful connect
        // that returned a version-mismatch).
        *store.first_warning.borrow_mut() = Some("stale daemon".into());
        assert_eq!(store.take_first_warning().as_deref(), Some("stale daemon"));
        // Second call returns None.
        assert_eq!(store.take_first_warning(), None);
    }

    #[test]
    fn no_warning_on_fresh_store() {
        let store = SessionStore::new(PathBuf::from("/nonexistent"));
        assert_eq!(store.take_first_warning(), None);
    }
}
