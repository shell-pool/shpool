//! The `Event` type — everything the `update` function can react to.

use crossterm::event::KeyEvent;
use shpool_protocol::Session;

/// Things that can happen to the TUI. The `update` function turns
/// each of these into a model mutation (and optionally a `Command`
/// for the executor to carry out).
#[derive(Debug)]
pub enum Event {
    /// A keystroke from crossterm. Wraps crossterm's own KeyEvent
    /// directly — `update` pattern-matches on key code + modifiers.
    Key(KeyEvent),

    /// The daemon answered a List request; here's the fresh data.
    SessionsRefreshed(Vec<Session>),

    /// The daemon list request failed. The string is a display-ready
    /// error message for the UI layer to surface.
    RefreshFailed(String),

    /// A child `shpool attach` process returned control to us. The
    /// `bool` is whether it exited cleanly — false means we should
    /// surface an error.
    AttachExited { ok: bool, name: String },

    /// A kill request to the daemon finished. Like AttachExited, we
    /// report failure as an error in the footer.
    KillFinished { ok: bool, name: String, err: Option<String> },
}
