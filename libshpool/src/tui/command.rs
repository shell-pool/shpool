//! `Command` — side effects the executor carries out after `update`.

/// A side effect the main loop should perform. Produced by `update`,
/// consumed by `mod.rs::execute`.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Refetch the session list from the daemon. Results come back as
    /// [`super::event::Event::SessionsRefreshed`] or
    /// [`super::event::Event::RefreshFailed`].
    Refresh,

    /// Spawn `shpool attach [-f] <name>` as a child process, suspend
    /// the TUI while it runs, and resume when it exits. Result comes
    /// back as [`super::event::Event::AttachExited`].
    Attach { name: String, force: bool },

    /// Like Attach but creates a brand-new session. In shpool this
    /// is the same daemon call as Attach (the daemon create-or-
    /// attaches), but we keep them distinct so update/view can
    /// enforce "name must not already exist" vs "name must already
    /// exist" pre-flight checks.
    Create(String),

    /// Kill the named session via the shpool protocol. Result comes
    /// back as [`super::event::Event::KillFinished`].
    Kill(String),

    /// Stop the main loop. No follow-up event.
    Quit,
}
