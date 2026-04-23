//! The pure view function. Given a Model, paint a Frame.
//!
//! "Pure" here means: no I/O, no terminal side-effects outside what
//! ratatui does via the Frame. That lets us snapshot-test with
//! ratatui's TestBackend — see the tests at the bottom.
//!
//! Layout (unbordered — content fills the screen):
//!
//! ```text
//!  shpool sessions (3)              <- title (BOLD), droppable on tight screens
//!    name   created  active         <- column header (DIM)
//! *>main    3d       2m             <- attached + selected (REVERSED via highlight_style)
//! *  build  2h       2h             <- attached elsewhere (`*` marker)
//!    notes  10m      10m            <- plain row
//!  [j] down  [k] up  [spc] attach  [n] new  [d] kill  [q] quit   <- footer
//! ```
//!
//! The row prefix is two ASCII columns: col 0 is `*` for sessions
//! attached elsewhere, col 1 is `>` for the currently-selected row.
//!
//! In modal states (CreateInput, ConfirmKill, ConfirmForce) the footer
//! is replaced by a prompt line. A transient `model.error` string
//! overrides all of the above.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
    Frame,
};
use shpool_protocol::Session;

use super::keymap;
use super::model::{is_attached, last_active_unix_ms, Mode, Model};

/// Draw one frame.
///
/// `now_ms` is the current wall-clock time in unix milliseconds. We
/// take it as a parameter so the relative-time rendering ("2m") is
/// deterministic — tests pass a fixed value, production passes
/// `SystemTime::now()` converted to ms.
pub fn view(model: &Model, now_ms: i64, frame: &mut Frame) {
    let total_area = frame.area();
    let widths = ColWidths::compute(&model.sessions, now_ms);

    // Adaptive chrome: the title row is the first thing dropped when
    // the terminal is tight (< 4 rows). Column header + list + footer
    // stay as long as possible. Usable-tui-under-2-rows isn't a real
    // scenario so no further cascade.
    let show_title = total_area.height >= 4;

    let mut constraints = Vec::with_capacity(4);
    if show_title {
        constraints.push(Constraint::Length(1)); // title
    }
    constraints.push(Constraint::Length(1)); // column header
    constraints.push(Constraint::Min(1)); // session list
    constraints.push(Constraint::Length(1)); // footer

    let chunks =
        Layout::default().direction(Direction::Vertical).constraints(constraints).split(total_area);

    let mut i = 0;
    if show_title {
        render_title(model, frame, chunks[i]);
        i += 1;
    }
    render_column_header(&widths, frame, chunks[i]);
    i += 1;
    render_sessions(model, now_ms, &widths, frame, chunks[i]);
    i += 1;
    render_footer(model, frame, chunks[i]);
}

/// Per-frame column widths: each column is sized to
/// `max(header.len(), widest value)`. Computed per frame so adding
/// or renaming sessions adapts without special-casing.
struct ColWidths {
    name: usize,
    created: usize,
    active: usize,
}

impl ColWidths {
    fn compute(sessions: &[Session], now_ms: i64) -> Self {
        let mut widths =
            Self { name: "name".len(), created: "created".len(), active: "active".len() };
        for s in sessions {
            widths.name = widths.name.max(s.name.chars().count());
            widths.created =
                widths.created.max(format_age(now_ms, s.started_at_unix_ms).chars().count());
            widths.active =
                widths.active.max(format_age(now_ms, last_active_unix_ms(s)).chars().count());
        }
        widths
    }
}

fn render_title(model: &Model, frame: &mut Frame, area: Rect) {
    let text = format!(" shpool sessions ({})", model.sessions.len());
    let p = Paragraph::new(Span::styled(text, Style::default().add_modifier(Modifier::BOLD)));
    frame.render_widget(p, area);
}

fn render_column_header(widths: &ColWidths, frame: &mut Frame, area: Rect) {
    // 2 leading spaces to line up with the `*>` row prefix.
    let text = format!(
        "  {name:<nw$}  {created:<cw$}  {active:<aw$}",
        name = "name",
        nw = widths.name,
        created = "created",
        cw = widths.created,
        active = "active",
        aw = widths.active,
    );
    let p = Paragraph::new(Span::styled(text, Style::default().add_modifier(Modifier::DIM)));
    frame.render_widget(p, area);
}

fn render_sessions(model: &Model, now_ms: i64, widths: &ColWidths, frame: &mut Frame, area: Rect) {
    if model.sessions.is_empty() {
        // Empty state: just say so. Key bindings show up in the
        // footer below; no point duplicating them here where they'd
        // drift if the keymap changes.
        let hint = Paragraph::new("no sessions");
        frame.render_widget(hint, area);
        return;
    }

    // Build one ListItem per session. ratatui diffs the resulting
    // buffer against the previous frame's, so this is cheap even for
    // long lists.
    let items: Vec<ListItem> = model
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| render_row(s, i == model.selected, widths, now_ms))
        .collect();

    // Stateful render: `ListState` tracks which row is selected; the
    // widget scrolls automatically so the selection stays on-screen
    // when the list is longer than the visible region.
    //
    // `highlight_style` applies to the whole selected line — the
    // REVERSED modifier makes it pop without needing per-span styling
    // inside `render_row`.
    let mut state = ListState::default().with_selected(Some(model.selected));
    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD));
    frame.render_stateful_widget(list, area, &mut state);
}

/// One row in the sessions list. Two-char ASCII prefix, then name
/// padded to the name column width, then two age columns.
fn render_row(s: &Session, selected: bool, widths: &ColWidths, now_ms: i64) -> ListItem<'static> {
    let attached_mark = if is_attached(s) { '*' } else { ' ' };
    let selected_mark = if selected { '>' } else { ' ' };
    let created = format_age(now_ms, s.started_at_unix_ms);
    let active = format_age(now_ms, last_active_unix_ms(s));

    // The line is plain-styled; whole-row reverse-video for the
    // selected row is applied by `List::highlight_style` above.
    let text = format!(
        "{a}{sel}{name:<nw$}  {created:<cw$}  {active:<aw$}",
        a = attached_mark,
        sel = selected_mark,
        name = s.name,
        nw = widths.name,
        created = created,
        cw = widths.created,
        active = active,
        aw = widths.active,
    );
    ListItem::new(text)
}

/// The bottom line: either key-binding hints (Normal mode), an edit
/// prompt (CreateInput), or a y/N confirmation (ConfirmKill /
/// ConfirmForce). Transient errors override all of the above —
/// they're the most important thing to see.
fn render_footer(model: &Model, frame: &mut Frame, area: Rect) {
    if let Some(err) = &model.error {
        let p = Paragraph::new(Span::styled(
            format!("! {err}"),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(p, area);
        return;
    }

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let line = match &model.mode {
        Mode::Normal => Line::from(footer_bindings_normal()),
        Mode::CreateInput(buf) => {
            // ASCII cursor (`_`) with no blink — some terminals
            // render blink poorly or ignore it entirely.
            let mut spans = vec![
                Span::styled("new session name: ", bold),
                Span::raw(buf.clone()),
                Span::raw("_"),
            ];
            spans.extend(hint_spans(keymap::CREATE_HINTS));
            Line::from(spans)
        }
        Mode::ConfirmKill(name) => {
            let mut spans = vec![Span::styled(format!("kill session '{name}'? "), bold)];
            spans.extend(hint_spans(keymap::CONFIRM_HINTS));
            Line::from(spans)
        }
        Mode::ConfirmForce(name) => {
            let mut spans = vec![Span::styled(
                format!("'{name}' is attached elsewhere — force attach? "),
                bold,
            )];
            spans.extend(hint_spans(keymap::CONFIRM_HINTS));
            Line::from(spans)
        }
    };

    frame.render_widget(Paragraph::new(line), area);
}

/// Build footer spans for Normal mode by iterating
/// [`super::keymap::NORMAL_BINDINGS`]. This is the view side of the single
/// source of truth — any change to bindings or labels in keymap.rs
/// is automatically reflected here.
fn footer_bindings_normal() -> Vec<Span<'static>> {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::raw(" ")];
    for binding in keymap::NORMAL_BINDINGS {
        spans.push(Span::styled(format!("[{}] ", binding.label), bold));
        spans.push(Span::raw(format!("{}  ", binding.desc)));
    }
    spans
}

/// Render a list of (label, desc) hint pairs as `[label] desc` spans
/// separated by spaces. Used for the modal-mode footers
/// (CreateInput, ConfirmKill, ConfirmForce).
fn hint_spans(hints: &'static [(&str, &str)]) -> Vec<Span<'static>> {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut spans = Vec::new();
    for (label, desc) in hints {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(format!("[{label}] "), bold));
        spans.push(Span::raw(*desc));
    }
    spans
}

/// Render a past millisecond-timestamp relative to `now_ms` as a
/// short string: "now", "42s", "13m", "4h", "9d". Past-only — future
/// timestamps (clock skew) render as "now".
///
/// Short format (no " ago" suffix) to keep the age columns compact.
/// The column headers ("created" / "active") supply the "ago" context.
fn format_age(now_ms: i64, then_ms: i64) -> String {
    // All arithmetic in i64 so clock skew doesn't panic.
    // `saturating_sub` clamps to 0 if then is in the future.
    let delta_s = now_ms.saturating_sub(then_ms) / 1000;
    if delta_s < 5 {
        "now".to_string()
    } else if delta_s < 60 {
        format!("{delta_s}s")
    } else if delta_s < 3600 {
        format!("{}m", delta_s / 60)
    } else if delta_s < 86_400 {
        format!("{}h", delta_s / 3600)
    } else {
        format!("{}d", delta_s / 86_400)
    }
}

// --- snapshot tests ---
//
// These use ratatui's TestBackend: an in-memory buffer we can
// compare to a stored golden file. `insta` handles the file I/O +
// review workflow (`cargo insta review` accepts/rejects changes).

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use shpool_protocol::SessionStatus;

    fn sess(name: &str, attached: bool, last_active_ms: i64) -> Session {
        Session {
            name: name.to_string(),
            started_at_unix_ms: last_active_ms,
            last_connected_at_unix_ms: Some(last_active_ms),
            last_disconnected_at_unix_ms: None,
            status: if attached { SessionStatus::Attached } else { SessionStatus::Disconnected },
        }
    }

    // Fixed "now" used by all view tests so relative-time rendering
    // is deterministic. 2026-01-15 22:30 UTC = 1768552200000.
    const NOW_MS: i64 = 1_768_552_200_000;

    fn render_to_string(model: &Model, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| view(model, NOW_MS, f)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn empty_list_shows_hint() {
        let m = Model::new();
        insta::assert_snapshot!(render_to_string(&m, 60, 6));
    }

    #[test]
    fn list_with_selection() {
        // Build two sessions with different ages so the relative-
        // time columns show varied output. "main" was active 2
        // minutes before NOW_MS; "build" 3 hours before.
        let mut m = Model::new();
        m.sessions = vec![
            sess("main", true, NOW_MS - 2 * 60 * 1000),
            sess("build", false, NOW_MS - 3 * 60 * 60 * 1000),
        ];
        m.selected = 1;
        insta::assert_snapshot!(render_to_string(&m, 70, 6));
    }

    #[test]
    fn confirm_kill_footer() {
        let mut m = Model::new();
        m.sessions = vec![sess("main", false, NOW_MS - 10 * 60 * 1000)];
        m.mode = Mode::ConfirmKill("main".into());
        insta::assert_snapshot!(render_to_string(&m, 70, 5));
    }

    #[test]
    fn error_replaces_footer() {
        let mut m = Model::new();
        m.sessions = vec![sess("main", false, NOW_MS - 30 * 1000)];
        m.set_error("daemon gone");
        insta::assert_snapshot!(render_to_string(&m, 60, 5));
    }

    #[test]
    fn confirm_force_footer() {
        let mut m = Model::new();
        m.sessions = vec![sess("main", true, NOW_MS - 5 * 60 * 1000)];
        m.mode = Mode::ConfirmForce("main".into());
        insta::assert_snapshot!(render_to_string(&m, 70, 5));
    }

    #[test]
    fn create_input_midtyping() {
        let mut m = Model::new();
        m.sessions = vec![sess("main", false, NOW_MS - 30 * 1000)];
        m.mode = Mode::CreateInput("foo".into());
        insta::assert_snapshot!(render_to_string(&m, 70, 5));
    }

    #[test]
    fn title_dropped_on_tight_screen() {
        // height 3 -> title should be dropped (column header + 1
        // list row + footer = 3). If the title is present, the list
        // has 0 rows and the selection disappears, so a grep for
        // "main" in the output tells us whether the session is
        // visible.
        let mut m = Model::new();
        m.sessions = vec![sess("main", false, NOW_MS - 30 * 1000)];
        let out = render_to_string(&m, 40, 3);
        assert!(out.contains("main"), "session row should be visible at h=3; got:\n{out}");
        assert!(!out.contains("shpool sessions"), "title should be dropped at h=3; got:\n{out}");
    }

    #[test]
    fn long_name_expands_column_not_clipped() {
        // Dynamic name-column width: a long session name should fit
        // without being clipped, and the column header should be
        // pushed out correspondingly. Guards against regressions if
        // `name_column_width` gets replaced with a fixed constant.
        let mut m = Model::new();
        m.sessions =
            vec![sess("a", false, NOW_MS), sess("very-long-session-name-here", false, NOW_MS)];
        let rendered = render_to_string(&m, 60, 6);
        assert!(
            rendered.contains("very-long-session-name-here"),
            "long name should not be clipped; got:\n{rendered}"
        );
        // The `created` and `active` headers should still be present
        // (i.e., the header row wasn't itself truncated).
        assert!(
            rendered.contains("created") && rendered.contains("active"),
            "column headers should survive the wide name; got:\n{rendered}"
        );
    }

    #[test]
    fn viewport_scrolls_to_keep_selection_visible() {
        // 50 sessions, screen height 10. If we select row 40, it
        // should still be visible in the rendered output thanks to
        // ratatui's ListState scrolling.
        let mut m = Model::new();
        m.sessions = (0..50).map(|i| sess(&format!("s{i}"), false, NOW_MS)).collect();
        m.selected = 40;
        let rendered = render_to_string(&m, 50, 10);
        assert!(
            rendered.contains("s40"),
            "selected session should stay on screen after scroll; got:\n{rendered}"
        );
    }

    #[test]
    fn format_age_buckets() {
        let now = 1_000_000_000_000i64;
        assert_eq!(format_age(now, now), "now");
        assert_eq!(format_age(now, now - 4_000), "now");
        assert_eq!(format_age(now, now - 42_000), "42s");
        assert_eq!(format_age(now, now - 3 * 60 * 1000), "3m");
        assert_eq!(format_age(now, now - 2 * 3600 * 1000), "2h");
        assert_eq!(format_age(now, now - 5 * 86_400 * 1000), "5d");
        // Future timestamps (clock skew) clamp to "now".
        assert_eq!(format_age(now, now + 999_999), "now");
    }
}
