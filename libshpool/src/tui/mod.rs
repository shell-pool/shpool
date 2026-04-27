//! `shpool tui` — interactive session manager.
//!
//! High-level architecture (elm-like):
//!
//!   event loop:
//!     1. draw the current Model (view.rs)
//!     2. read the next Event (key, or a follow-up from a Command)
//!     3. fold Event into Model, possibly yielding a Command (update.rs)
//!     4. if there's a Command, execute it; its result becomes the next Event
//!
//! The split is deliberate: `update` is a pure function and trivially
//! tested; `view` is pure and snapshot-tested; all side effects
//! (socket, subprocess, terminal) live in this file's `execute`.

mod attach;
mod command;
mod event;
mod keymap;
mod model;
mod store;
mod suspend;
mod update;
mod view;

use std::{
    env, io,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use crossterm::{
    cursor::Hide,
    event::{self as xterm_event, DisableFocusChange, EnableFocusChange, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use shpool_protocol::Session;

use self::{
    attach::AttachEnv,
    command::Command,
    event::Event,
    model::{is_attached, Mode, Model},
    store::SessionStore,
};

/// `config_file`, `log_file`, and `verbose` are the parent invocation's
/// top-level flags, forwarded to every `shpool attach` subprocess
/// spawned from the TUI so those children see the same config /
/// logging destination / verbosity as the TUI itself.
pub fn run(
    socket: PathBuf,
    config_file: Option<String>,
    log_file: Option<String>,
    verbose: u8,
) -> Result<()> {
    // Refuse to run inside a shpool session. Nested sessions are
    // confusing: a force-attach from inside bumps the outer client,
    // SHPOOL_SESSION_NAME ends up inherited, and ^D leaves the user
    // at the wrong layer. Print a hint and exit rather than open the
    // TUI — the user will notice and detach first.
    if let Ok(name) = env::var("SHPOOL_SESSION_NAME") {
        eprintln!(
            "shpool tui: refusing to run inside shpool session '{name}'.\n\
             Run `shpool detach` first."
        );
        return Ok(());
    }

    // Bring up the terminal. Failure here is fatal — we can't do
    // anything useful without it — but we make sure to tear it back
    // down cleanly on any error path so the user's shell doesn't get
    // left in raw mode.
    let mut terminal = enter_tui().context("entering TUI")?;

    let attach_env = AttachEnv { socket, config_file, log_file, verbose };

    // The actual loop is in its own function so we can always run
    // `leave_tui` on exit, even on error.
    let result = main_loop(&mut terminal, &attach_env);

    // Always tear down. If `result` is already an error we'll
    // propagate that; teardown errors go to stderr after the terminal
    // is restored so the user can read them.
    if let Err(e) = leave_tui(terminal) {
        eprintln!("shpool tui: error restoring terminal: {e:?}");
    }

    result
}

/// Set up the terminal for TUI use. Returns a ratatui `Terminal`
/// bound to stdout.
fn enter_tui() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("enabling raw mode")?;
    // EnableFocusChange is best-effort: terminals that don't implement
    // the escape sequence just ignore it, and we never receive
    // FocusGained events from them — benign no-op.
    execute!(io::stdout(), EnterAlternateScreen, Hide, EnableFocusChange)
        .context("entering alt screen")?;
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend).context("creating terminal")?;
    Ok(terminal)
}

/// Reverse of `enter_tui`. We consume the Terminal because after this
/// it's invalid to draw to it.
fn leave_tui(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    // Clear + show cursor first so the user's shell prompt lands in
    // a sensible place.
    use crossterm::cursor::Show;
    execute!(terminal.backend_mut(), DisableFocusChange, LeaveAlternateScreen, Show)
        .context("leaving alt screen")?;
    disable_raw_mode().context("disabling raw mode")?;
    Ok(())
}

/// The event loop. Runs until the model flags `quit` or we hit an
/// unrecoverable error.
fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    attach_env: &AttachEnv,
) -> Result<()> {
    // SessionStore only needs the socket — it doesn't care about the
    // forwarded flags. Clone the PathBuf (cheap, one-time) rather than
    // threading a lifetime through SessionStore.
    let store = SessionStore::new(attach_env.socket.clone());
    let mut model = Model::new();

    // Initial fetch: if the daemon is alive, show its session list
    // immediately; if not, show the error and let the user try
    // again (quit + retry).
    //
    // We feed the result through `update` rather than mutating model
    // directly so the error-handling path stays in one place.
    let initial = match store.list() {
        Ok(sessions) => Event::SessionsRefreshed(sessions),
        Err(e) => Event::RefreshFailed(format!("{e:#}")),
    };
    update::update(&mut model, initial);

    // Surface the first protocol-version-mismatch warning (if any) in
    // the footer so the user learns about daemon/client drift once.
    // Subsequent connects may hit the same daemon and re-surface the
    // warning; we only show it on the first occurrence.
    if let Some(warning) = store.take_first_warning() {
        model.set_error(format!("warning: {warning}"));
    }

    loop {
        // 1. Render the current state. `now_ms` drives the relative-
        // time column in the session list ("2m" etc.); passing it
        // in from here (rather than having view.rs call
        // SystemTime::now itself) keeps view pure + snapshot-testable.
        let now = now_ms();
        terminal.draw(|f| view::view(&model, now, f)).context("drawing frame")?;

        // `quit` is checked AFTER the draw, not before. The final
        // pass is technically a wasted frame, but LeaveAlternateScreen
        // wipes it on teardown so the user never sees it. Deliberate —
        // inverting to check-first would subtly change which frame
        // the model's visible state matches, and the saved work
        // (one draw on exit) isn't worth that coupling.
        if model.quit {
            return Ok(());
        }

        // 2. Read the next key. crossterm handles SIGWINCH for us —
        // a resize shows up as `Event::Resize`, which we treat as a
        // no-op (the next draw picks up the new size).
        let next = read_one_event().context("reading input")?;

        // 3. Fold it into the model and maybe get a Command back.
        let mut next_cmd = update::update(&mut model, next);

        // 4. Execute the Command, possibly producing a follow-up
        // Event we feed back into update. If update produces ANOTHER
        // Command from that follow-up, we execute that too, and so
        // on. This cascade is load-bearing: e.g. create flow is
        //   Key(Enter) -> Create -> AttachExited -> Refresh ->
        //   SessionsRefreshed
        // where the Refresh step is the one that picks up the newly
        // created session. If we only executed the first Command
        // (Create), the new session would never appear.
        //
        // Drift note: the specific command/event wiring (e.g.
        // "AttachExited produces Refresh") lives in
        // `update::update`'s match arms — this comment is the
        // narrative but not the source of truth. If that wiring
        // changes, update this example too.
        while let Some(cmd) = next_cmd.take() {
            let follow_up = execute(cmd, &mut model, &store, terminal, attach_env)?;
            let Some(ev) = follow_up else { break };
            next_cmd = update::update(&mut model, ev);
        }
    }
}

/// Outcome of an attach pre-flight: what did a fresh `list` say
/// about the session the user wants to attach to?
///
/// Split out from the [`Command::Attach`] executor arm because the
/// decision is meaningfully separate from the action. Each variant
/// maps to one response in `execute`.
///
/// Why pre-flight at all: the model's view of "is this session
/// attached elsewhere" can be stale because we only refresh on
/// keystroke, and even after a refresh the daemon's own state can
/// lag by ~0.5s (e.g. the user detached from another terminal and
/// the daemon hasn't reflected it yet). We want that stale state
/// to flash through without popping a spurious ConfirmForce
/// prompt, so we re-check with fresh data right before acting.
enum AttachPreflight {
    /// `store.list()` itself failed — the error is display-ready.
    RefreshFailed(anyhow::Error),
    /// Session no longer exists; another client killed it since
    /// our last view. The fresh list is included so the caller can
    /// feed it through [`Event::SessionsRefreshed`] — which routes
    /// to [`Model::apply_refresh`] via [`update::update`].
    Gone { sessions: Vec<Session> },
    /// Session exists but is attached from another terminal, and
    /// force was not set. Caller should pop a ConfirmForce prompt.
    AttachedElsewhere { sessions: Vec<Session> },
    /// Session exists and is ready to attach. No `sessions` carried —
    /// the AttachExited handler will cascade into a fresh Refresh.
    ClearToAttach,
}

fn preflight_attach(store: &SessionStore, name: &str, force: bool) -> AttachPreflight {
    let sessions = match store.list() {
        Ok(s) => s,
        Err(e) => return AttachPreflight::RefreshFailed(e),
    };
    // Two scans over the list: one to check presence (so we can move
    // `sessions` into Gone without holding a borrow), one to check
    // attached-state on the matching entry. Scan cost is trivial at
    // session-list sizes. Structuring as a single-scan `.find()`
    // creates a borrow that conflicts with moving `sessions` into
    // the outcome variant.
    if !sessions.iter().any(|s| s.name == name) {
        return AttachPreflight::Gone { sessions };
    }
    let attached_elsewhere =
        !force && sessions.iter().find(|s| s.name == name).map(is_attached).unwrap_or(false);
    if attached_elsewhere {
        return AttachPreflight::AttachedElsewhere { sessions };
    }
    AttachPreflight::ClearToAttach
}

/// Current wall-clock time in unix milliseconds. Saturates to 0 if
/// the clock is before the unix epoch (shouldn't happen in practice,
/// but don't panic over it).
fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Block until crossterm delivers something we want to feed into
/// update. Resize events are absorbed here — we don't bother the
/// model with them.
///
/// Returns `Event::Key(...)` or `Event::FocusGained`. RefreshFailed /
/// AttachExited / etc. all come from the executor; having the
/// read-key path return the same `Event` type as the executor keeps
/// the main loop simple.
fn read_one_event() -> Result<Event> {
    loop {
        match xterm_event::read()? {
            // KeyEventKind::Press — only real presses, not releases
            // or repeats. Some terminals send KeyEventKind::Release
            // on kitty protocol; we'd double-fire without this
            // filter.
            xterm_event::Event::Key(k) if k.kind == KeyEventKind::Press => {
                return Ok(Event::Key(k));
            }
            xterm_event::Event::FocusGained => return Ok(Event::FocusGained),
            // Resize: the next `terminal.draw` will pick up the new
            // size on its own. Loop around to read another event.
            xterm_event::Event::Resize(_, _) => continue,
            // Mouse / paste / focus-lost / key-release: ignore.
            _ => continue,
        }
    }
}

/// Side-effect executor. Takes a Command from `update`, does the
/// thing, and returns the follow-up Event (if any) for the main loop
/// to feed back into update.
///
/// All IO (socket, subprocess, terminal suspend) happens here. This
/// is the layer you look at when debugging "why didn't my kill
/// work" — all three possibilities (bad protocol call, silent daemon
/// error, botched UI state refresh) are visible in this function.
fn execute(
    cmd: Command,
    model: &mut Model,
    store: &SessionStore,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    attach_env: &AttachEnv,
) -> Result<Option<Event>> {
    match cmd {
        Command::Quit => {
            model.quit = true;
            Ok(None)
        }
        Command::Refresh => {
            let t0 = Instant::now();
            let ev = match store.list() {
                Ok(sessions) => Event::SessionsRefreshed(sessions),
                Err(e) => Event::RefreshFailed(format!("{e:#}")),
            };
            // Log slow daemon responses so we can spot regressions
            // without having to repro by eye. tracing integrates
            // with libshpool's existing log setup.
            let elapsed = t0.elapsed();
            if elapsed > Duration::from_millis(200) {
                tracing::warn!(?elapsed, "tui: slow shpool list");
            }
            Ok(Some(ev))
        }
        Command::Attach { name, force } => match preflight_attach(store, &name, force) {
            AttachPreflight::RefreshFailed(e) => {
                // Route through Event::RefreshFailed so the
                // "shpool list:" prefix is applied in exactly one
                // place (update's RefreshFailed handler), not
                // duplicated here.
                Ok(Some(Event::RefreshFailed(format!("{e:#}"))))
            }
            AttachPreflight::Gone { sessions } => {
                model.set_error(format!("session '{name}' is gone"));
                Ok(Some(Event::SessionsRefreshed(sessions)))
            }
            AttachPreflight::AttachedElsewhere { sessions } => {
                // Pop the confirm-force prompt rather than bumping the
                // other client silently. The user can press 'y' to
                // re-issue Attach with force=true, which skips the
                // preflight attached-elsewhere check.
                model.mode = Mode::ConfirmForce(name);
                Ok(Some(Event::SessionsRefreshed(sessions)))
            }
            AttachPreflight::ClearToAttach => {
                // Skip apply_refresh here — the AttachExited handler
                // cascades into a fresh Refresh anyway, and we don't
                // draw between here and the suspend, so any
                // intermediate refresh state would be invisible.
                let ok = suspend::with_tui_suspended(terminal, || {
                    attach::spawn_attach(&name, force, attach_env)
                })?;
                Ok(Some(Event::AttachExited { ok, name }))
            }
        },
        Command::Create(name) => {
            // `shpool attach` with a fresh name creates + attaches
            // atomically on the daemon side. We pre-flight the
            // "already exists" check in update (see CreateInput
            // Enter handler) so by the time we get here it's safe
            // to just spawn.
            let ok = suspend::with_tui_suspended(terminal, || {
                attach::spawn_attach(&name, false, attach_env)
            })?;
            Ok(Some(Event::AttachExited { ok, name }))
        }
        Command::Kill(name) => {
            let result = store.kill(&name);
            let (ok, err) = match result {
                Ok(()) => (true, None),
                Err(e) => (false, Some(format!("kill {name}: {e:#}"))),
            };
            Ok(Some(Event::KillFinished { ok, name, err }))
        }
    }
}
