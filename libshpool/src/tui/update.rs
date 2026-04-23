//! The update function: `(&mut Model, Event) -> Option<Command>`.
//!
//! Deterministic and free of I/O — no socket calls, no terminal
//! writes, no subprocess spawns. It reads the event, mutates the
//! supplied model in place, and optionally emits a `Command` for
//! the executor to carry out.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::command::Command;
use super::event::Event;
use super::keymap::{is_dispatchable, normal_action, NormalAction};
use super::model::{Mode, Model};

/// Fold one event into the model. Returns `Some(Command)` if the
/// event triggers a side effect (attach / kill / refresh / quit).
pub fn update(model: &mut Model, event: Event) -> Option<Command> {
    // Any non-async event (i.e. a keystroke) clears the transient
    // error. Async events (refresh finishing, kill finishing) can
    // SET the error but shouldn't clear one the user hasn't seen.
    let was_keystroke = matches!(event, Event::Key(_));
    if was_keystroke {
        model.error = None;
    }

    let cmd = match event {
        Event::Key(key) => handle_key(model, key),
        Event::SessionsRefreshed(sessions) => {
            model.apply_refresh(sessions);
            None
        }
        Event::RefreshFailed(msg) => {
            model.set_error(format!("shpool list: {msg}"));
            None
        }
        Event::AttachExited { ok, name } => {
            // Attach teardown is our chance to freshen the list —
            // the session we just attached to may have changed state
            // and other clients may have created/killed sessions
            // while we were suspended.
            if !ok {
                model.set_error(format!("shpool attach {name} failed"));
            }
            // Reselect the session we just attached to, so when the
            // user comes back they're looking at what they just left.
            if let Some(i) = model.sessions.iter().position(|s| s.name == name) {
                model.selected = i;
            }
            Some(Command::Refresh)
        }
        Event::KillFinished { ok, name, err } => {
            if !ok {
                let msg = err.unwrap_or_else(|| format!("kill {name} failed"));
                model.set_error(msg);
            }
            Some(Command::Refresh)
        }
    };

    // Auto-refresh: when a Normal-mode keystroke finishes without
    // producing its own Command, request a refresh so the session
    // list tracks daemon-side changes (sessions created/killed/
    // detached by other clients) without needing explicit user
    // action. We skip this in modal modes so typing "foo" in
    // CreateInput isn't three socket round-trips.
    if was_keystroke && cmd.is_none() && matches!(model.mode, Mode::Normal) {
        return Some(Command::Refresh);
    }
    cmd
}

/// Dispatch a single key event based on the current mode.
fn handle_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    // Ctrl-C is a global quit regardless of mode — matches most
    // interactive tools. Checking it here means we don't have to
    // duplicate the handler in every mode branch.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Command::Quit);
    }

    match &mut model.mode {
        Mode::Normal => handle_key_normal(model, key),
        Mode::CreateInput(_) => handle_key_create(model, key),
        Mode::ConfirmKill(_) => handle_key_confirm_kill(model, key),
        Mode::ConfirmForce(_) => handle_key_confirm_force(model, key),
    }
}

fn handle_key_normal(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let Some(action) = normal_action(&key) else { return None };

    match action {
        NormalAction::SelectPrev => {
            model.select_prev();
            None
        }
        NormalAction::SelectNext => {
            model.select_next();
            None
        }
        NormalAction::AttachSelected => {
            // We DON'T make the "is it already attached?" call
            // here — the model's view of attached-state might be
            // stale (e.g. the user just detached elsewhere ~0.5s
            // ago and the daemon hasn't reflected it yet). Just
            // emit Command::Attach and let the executor make that
            // decision with fresh data right before spawning.
            let Some(session) = model.sessions.get(model.selected) else {
                return None;
            };
            Some(Command::Attach { name: session.name.clone(), force: false })
        }
        NormalAction::NewSession => {
            model.mode = Mode::CreateInput(String::new());
            None
        }
        NormalAction::KillSelected => {
            let Some(session) = model.sessions.get(model.selected) else {
                return None;
            };
            model.mode = Mode::ConfirmKill(session.name.clone());
            None
        }
        NormalAction::Quit => Some(Command::Quit),
    }
}

fn handle_key_create(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let Mode::CreateInput(buf) = &mut model.mode else {
        // Unreachable because handle_key already matched Mode, but
        // the compiler doesn't know that — we have to destructure
        // again to access the buffer.
        return None;
    };

    match key.code {
        // Enter submits.
        KeyCode::Enter => {
            let name = std::mem::take(buf);
            model.mode = Mode::Normal;
            if name.is_empty() {
                return None;
            }
            // Reject duplicates here rather than in the executor so
            // the error surfaces in the TUI footer immediately
            // instead of after a daemon round-trip.
            if model.sessions.iter().any(|s| s.name == name) {
                model.set_error(format!("session '{name}' already exists"));
                return None;
            }
            Some(Command::Create(name))
        }
        KeyCode::Esc => {
            model.mode = Mode::Normal;
            None
        }
        KeyCode::Backspace => {
            buf.pop();
            None
        }
        // Any printable character gets appended. We skip:
        //   - chars with CONTROL or ALT modifiers (so Ctrl-X doesn't land in the name)
        //   - whitespace (space, tab)
        KeyCode::Char(c)
            if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && !c.is_whitespace() =>
        {
            buf.push(c);
            None
        }
        _ => None,
    }
}

fn handle_key_confirm_kill(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let Mode::ConfirmKill(name) = &mut model.mode else {
        return None;
    };
    // y/Y confirms; n/N/Enter/Esc cancel. Any other key leaves the
    // prompt open. Enter/Esc are convenience aliases not shown in
    // the `[y/N]` hint. Chord keys are filtered via `is_dispatchable`.
    if !is_dispatchable(&key) {
        return None;
    }
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let name = std::mem::take(name);
            model.mode = Mode::Normal;
            Some(Command::Kill(name))
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
            model.mode = Mode::Normal;
            None
        }
        _ => None,
    }
}

fn handle_key_confirm_force(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let Mode::ConfirmForce(name) = &mut model.mode else {
        return None;
    };
    if !is_dispatchable(&key) {
        return None;
    }
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let name = std::mem::take(name);
            model.mode = Mode::Normal;
            Some(Command::Attach { name, force: true })
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
            model.mode = Mode::Normal;
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::keymap::CONFIRM_HINTS;
    use super::*;
    use shpool_protocol::{Session, SessionStatus};

    // Helper: build a Session with just the fields our logic looks at.
    fn session(name: &str, attached: bool) -> Session {
        Session {
            name: name.to_string(),
            started_at_unix_ms: 0,
            last_connected_at_unix_ms: None,
            last_disconnected_at_unix_ms: None,
            status: if attached { SessionStatus::Attached } else { SessionStatus::Disconnected },
        }
    }

    // Helper: build a KeyEvent with no modifiers.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn space_on_detached_session_attaches() {
        let mut m = Model::new();
        m.sessions = vec![session("a", false)];
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char(' '))));
        assert_eq!(cmd, Some(Command::Attach { name: "a".into(), force: false }));
    }

    #[test]
    fn enter_also_attaches() {
        let mut m = Model::new();
        m.sessions = vec![session("a", false)];
        let cmd = update(&mut m, Event::Key(key(KeyCode::Enter)));
        assert_eq!(cmd, Some(Command::Attach { name: "a".into(), force: false }));
    }

    #[test]
    fn space_on_attached_session_still_emits_attach() {
        // The ConfirmForce decision moved to the executor (which
        // refreshes first to avoid stale data). update unconditionally
        // emits Command::Attach { force: false }; executor may turn
        // it into a ConfirmForce mode transition if fresh data agrees
        // the session is attached elsewhere.
        let mut m = Model::new();
        m.sessions = vec![session("a", true)];
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char(' '))));
        assert_eq!(cmd, Some(Command::Attach { name: "a".into(), force: false }));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn y_in_confirm_kill_issues_kill() {
        let mut m = Model::new();
        m.sessions = vec![session("a", false)];
        m.mode = Mode::ConfirmKill("a".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('y'))));
        assert_eq!(cmd, Some(Command::Kill("a".into())));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn n_in_confirm_kill_cancels() {
        let mut m = Model::new();
        m.mode = Mode::ConfirmKill("a".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('n'))));
        // Cancel returns to Normal mode; auto-refresh then kicks in.
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn enter_in_confirm_kill_cancels() {
        // Enter in a `[y/N]` prompt = accept the default (N) = cancel.
        let mut m = Model::new();
        m.mode = Mode::ConfirmKill("a".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Enter)));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn esc_in_confirm_kill_cancels() {
        let mut m = Model::new();
        m.mode = Mode::ConfirmKill("a".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Esc)));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn unrelated_key_in_confirm_kill_stays_open() {
        // 'x' is not in {y, Y, n, N, Enter, Esc}. Should NOT dismiss
        // the prompt — prevents stray keys from accidentally closing
        // the modal.
        let mut m = Model::new();
        m.mode = Mode::ConfirmKill("a".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('x'))));
        assert_eq!(cmd, None, "stray key should not trigger a command");
        assert_eq!(m.mode, Mode::ConfirmKill("a".into()), "prompt should still be open");
    }

    #[test]
    fn y_in_confirm_force_force_attaches() {
        let mut m = Model::new();
        m.mode = Mode::ConfirmForce("main".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('y'))));
        assert_eq!(cmd, Some(Command::Attach { name: "main".into(), force: true }));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn unrelated_key_in_confirm_force_stays_open() {
        let mut m = Model::new();
        m.mode = Mode::ConfirmForce("main".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('x'))));
        assert_eq!(cmd, None);
        assert_eq!(m.mode, Mode::ConfirmForce("main".into()));
    }

    #[test]
    fn create_rejects_duplicate() {
        let mut m = Model::new();
        m.sessions = vec![session("main", false)];
        m.mode = Mode::CreateInput("main".into());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Enter)));
        // Reject-and-return-to-Normal triggers the auto-refresh.
        assert_eq!(cmd, Some(Command::Refresh));
        assert!(m.error.as_deref().unwrap_or("").contains("already exists"));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn create_typing_accumulates_into_buffer() {
        let mut m = Model::new();
        m.mode = Mode::CreateInput(String::new());
        update(&mut m, Event::Key(key(KeyCode::Char('f'))));
        update(&mut m, Event::Key(key(KeyCode::Char('o'))));
        update(&mut m, Event::Key(key(KeyCode::Char('o'))));
        assert_eq!(m.mode, Mode::CreateInput("foo".into()));
    }

    #[test]
    fn create_rejects_whitespace_in_name() {
        // Typing a space in CreateInput is a no-op — shpool stores
        // the name verbatim into env vars / prompt prefixes, and
        // spaces cause downstream pain.
        let mut m = Model::new();
        m.mode = Mode::CreateInput("foo".into());
        update(&mut m, Event::Key(key(KeyCode::Char(' '))));
        update(&mut m, Event::Key(key(KeyCode::Char('\t'))));
        assert_eq!(m.mode, Mode::CreateInput("foo".into()));
    }

    #[test]
    fn keystroke_clears_transient_error() {
        let mut m = Model::new();
        m.set_error("boom");
        update(&mut m, Event::Key(key(KeyCode::Char('j'))));
        assert!(m.error.is_none());
    }

    #[test]
    fn ctrl_c_quits_in_any_mode() {
        let mut m = Model::new();
        m.mode = Mode::CreateInput("half-typed".into());
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(update(&mut m, Event::Key(ctrl_c)), Some(Command::Quit));
    }

    #[test]
    fn navigation_keystroke_triggers_auto_refresh() {
        // In Normal mode, any no-op-ish keystroke (j/k/etc.) should
        // request a Refresh so the list stays current without the
        // user having to do anything explicit.
        let mut m = Model::new();
        m.sessions = vec![session("a", false), session("b", false)];
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('j'))));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.selected, 1);
    }

    #[test]
    fn kill_on_empty_list_is_noop() {
        // Pressing `d` on an empty list should not enter ConfirmKill
        // mode (no session to confirm against). The `get(selected)`
        // pre-check in handle_key_normal defends against this — lock
        // it in so a future refactor doesn't regress.
        let mut m = Model::new();
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('d'))));
        // No session-binding command, but the keystroke still triggers
        // the auto-refresh since we're in Normal mode with no cmd.
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn attach_on_empty_list_is_noop() {
        // Same shape as kill_on_empty_list_is_noop — pressing space
        // (or Enter) on an empty list must not emit Command::Attach
        // with an empty name.
        let mut m = Model::new();
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char(' '))));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn uppercase_letters_dispatch_same_as_lowercase() {
        // Shift-J should move the selection down, just like j.
        // keymap::NORMAL_BINDINGS enumerates both cases as synonyms.
        let mut m = Model::new();
        m.sessions = vec![session("a", false), session("b", false)];
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('J'))));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.selected, 1);
    }

    #[test]
    fn ctrl_d_does_not_kill() {
        // Ctrl-D is a shell-reflex keypress; it should NOT enter the
        // kill-confirmation prompt even though 'd' alone would.
        // `keymap::normal_action` filters out CONTROL-chord presses.
        let mut m = Model::new();
        m.sessions = vec![session("a", false)];
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let cmd = update(&mut m, Event::Key(ctrl_d));
        // No command from the binding → auto-refresh kicks in.
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn alt_j_does_not_navigate() {
        // Alt-J is a chord; it should NOT move the selection.
        let mut m = Model::new();
        m.sessions = vec![session("a", false), session("b", false)];
        let alt_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT);
        let cmd = update(&mut m, Event::Key(alt_j));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn ctrl_n_does_not_open_create_mode() {
        let mut m = Model::new();
        let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        let cmd = update(&mut m, Event::Key(ctrl_n));
        assert_eq!(cmd, Some(Command::Refresh));
        assert_eq!(m.mode, Mode::Normal);
    }

    #[test]
    fn refresh_event_does_not_trigger_another_refresh() {
        // Guards against an infinite-refresh loop: SessionsRefreshed
        // feeds back into update() but must not itself produce
        // Command::Refresh. The `was_keystroke` gate in update.rs
        // exists for exactly this reason.
        let mut m = Model::new();
        let cmd = update(&mut m, Event::SessionsRefreshed(vec![]));
        assert_eq!(cmd, None);
    }

    #[test]
    fn refresh_failed_event_does_not_trigger_another_refresh() {
        // Same invariant for the failure path — otherwise a down
        // daemon would produce a tight refresh-loop of failures.
        let mut m = Model::new();
        let cmd = update(&mut m, Event::RefreshFailed("boom".into()));
        assert_eq!(cmd, None);
    }

    #[test]
    fn typing_in_create_mode_does_not_auto_refresh() {
        // Auto-refresh is skipped in modal modes so typing a name
        // isn't a per-keystroke socket round-trip.
        let mut m = Model::new();
        m.mode = Mode::CreateInput(String::new());
        let cmd = update(&mut m, Event::Key(key(KeyCode::Char('f'))));
        assert_eq!(cmd, None);
    }

    // Every key advertised in the CONFIRM_HINTS label must actually
    // be dispatched (not fall through to the stay-open catch-all).
    // Catches drift where the hint promises a key that the match
    // arms no longer accept.
    fn hint_chars() -> impl Iterator<Item = char> {
        CONFIRM_HINTS.iter().flat_map(|(label, _)| {
            label.split('/').filter_map(|piece| piece.chars().next()).collect::<Vec<_>>()
        })
    }

    #[test]
    fn confirm_kill_dispatches_every_hint_key() {
        for c in hint_chars() {
            let mut m = Model::new();
            m.mode = Mode::ConfirmKill("a".into());
            update(&mut m, Event::Key(key(KeyCode::Char(c))));
            assert_ne!(
                m.mode,
                Mode::ConfirmKill("a".into()),
                "hint advertises '{c}' but dispatch leaves ConfirmKill open",
            );
        }
    }

    #[test]
    fn confirm_force_dispatches_every_hint_key() {
        for c in hint_chars() {
            let mut m = Model::new();
            m.mode = Mode::ConfirmForce("main".into());
            update(&mut m, Event::Key(key(KeyCode::Char(c))));
            assert_ne!(
                m.mode,
                Mode::ConfirmForce("main".into()),
                "hint advertises '{c}' but dispatch leaves ConfirmForce open",
            );
        }
    }
}
