//! The TUI's "model" — the plain-data state that the view reads from
//! and that `update` mutates. Everything in here is pure data + pure
//! methods; no I/O, no terminal, no sockets.

use shpool_protocol::{Session, SessionStatus};

/// Which modal "screen" the TUI is currently in.
#[derive(Debug, PartialEq, Default)]
pub enum Mode {
    /// Default view: session list with selection.
    #[default]
    Normal,

    /// The user is naming a new session. The `String` is the
    /// in-progress edit buffer.
    CreateInput(String),

    /// The user is confirming whether to kill a session. The `String`
    /// is the session name. (Key dispatch — which keys confirm,
    /// cancel, or are ignored — lives in `update`.)
    ConfirmKill(String),

    /// Attach pre-flight found the session attached elsewhere; the
    /// user is deciding whether to force-attach (which bumps the
    /// other client). The `String` is the session name. (Key dispatch
    /// lives in `update`.)
    ConfirmForce(String),
}

/// Everything the view needs to render a frame, plus the cursor
/// position in the list.
#[derive(Default)]
pub struct Model {
    /// The full session list, sorted most-recently-active first.
    pub sessions: Vec<Session>,

    /// Index into `sessions` of the currently-highlighted row. Stays
    /// valid (< sessions.len()) unless `sessions` is empty, in which
    /// case it's 0 and the view renders "no sessions".
    pub selected: usize,

    /// Which modal screen is active.
    pub mode: Mode,

    /// Transient error message shown in the footer until the next
    /// keystroke.
    pub error: Option<String>,

    /// Set to true when the user's triggered a Quit command. The main
    /// loop checks this after each `update` and exits if set.
    pub quit: bool,
}

impl Model {
    /// Construct an empty model. Delegates to `Default::default()` —
    /// kept as an explicit constructor for call-site clarity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Name of the currently-selected session, or None if the list is
    /// empty.
    pub fn selected_name(&self) -> Option<&str> {
        self.sessions.get(self.selected).map(|s| s.name.as_str())
    }

    /// Move selection down, wrapping at the bottom. No-op if empty.
    pub fn select_next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.sessions.len();
    }

    /// Move selection up, wrapping at the top.
    pub fn select_prev(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.sessions.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Replace the session list, preserving selection by name when
    /// possible. If the previously-selected session disappeared, the
    /// old index is clamped into the new list. Sessions are sorted
    /// most-recently-active first.
    pub fn apply_refresh(&mut self, mut new_sessions: Vec<Session>) {
        new_sessions.sort_by_key(|s| std::cmp::Reverse(last_active_unix_ms(s)));

        // Capture old selection before replacing the list.
        let prev_name = self.selected_name().map(str::to_string);
        let prev_idx = self.selected;

        self.sessions = new_sessions;

        // saturating_sub avoids underflow when sessions is empty.
        self.selected = prev_name
            .and_then(|name| self.sessions.iter().position(|s| s.name == name))
            .unwrap_or_else(|| prev_idx.min(self.sessions.len().saturating_sub(1)));
    }

    /// Set a transient error. The next keystroke clears it (handled
    /// in `update`).
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }
}

// Free helpers over shpool_protocol::Session. The orphan rule blocks
// inherent impls on foreign types, so we use free functions.

/// Unix ms of the most recent state transition — the newer of
/// last-connected and last-disconnected, falling back to creation
/// time. Used for "last-active" sorting.
pub fn last_active_unix_ms(s: &Session) -> i64 {
    s.last_connected_at_unix_ms
        .unwrap_or(0)
        .max(s.last_disconnected_at_unix_ms.unwrap_or(0))
        .max(s.started_at_unix_ms)
}

/// Whether the session currently has a client attached (from the
/// daemon's perspective).
pub fn is_attached(s: &Session) -> bool {
    matches!(s.status, SessionStatus::Attached)
}
