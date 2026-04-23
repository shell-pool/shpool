//! Key binding tables — single source of truth for both dispatch and
//! footer help text.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The set of logical actions bound to keys in Normal mode.
///
/// Why an enum rather than function pointers in the binding table:
/// some actions need to read the current model state (e.g., "attach
/// the selected session" needs `model.sessions[model.selected]`)
/// and some are stateless (e.g., "select previous"). An enum lets
/// update.rs match on the action and do whatever state-access each
/// variant needs. The compiler's exhaustiveness check then enforces
/// that every variant is handled — no silent drift when we add a
/// new action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalAction {
    SelectPrev,
    SelectNext,
    AttachSelected,
    NewSession,
    KillSelected,
    Quit,
}

/// One entry in the Normal-mode binding table.
pub struct NormalBinding {
    /// Short key label shown in the footer (e.g. "j", "spc").
    pub label: &'static str,
    /// Description shown in the footer (e.g. "down", "attach").
    pub desc: &'static str,
    /// Every KeyCode that triggers this action. All entries get the
    /// same label+desc — useful for binding arrows + vim letters to
    /// the same action without cluttering the footer.
    pub keys: &'static [KeyCode],
    pub action: NormalAction,
}

/// Normal-mode bindings. Order here is the order shown in the footer.
///
/// Case synonyms (`'j'` and `'J'`) are listed explicitly — there's no
/// automatic case folding at lookup time. Treating Shift-letters as
/// the same action is then just a matter of whether you enumerate the
/// uppercase variant here. If a future binding wants distinct uppercase
/// semantics (Vim-style `G` = bottom), remove the uppercase entry from
/// the synonym list and add a separate binding for it.
pub const NORMAL_BINDINGS: &[NormalBinding] = &[
    NormalBinding {
        label: "j",
        desc: "down",
        keys: &[KeyCode::Char('j'), KeyCode::Char('J'), KeyCode::Down],
        action: NormalAction::SelectNext,
    },
    NormalBinding {
        label: "k",
        desc: "up",
        keys: &[KeyCode::Char('k'), KeyCode::Char('K'), KeyCode::Up],
        action: NormalAction::SelectPrev,
    },
    NormalBinding {
        label: "spc",
        desc: "attach",
        keys: &[KeyCode::Char(' '), KeyCode::Enter],
        action: NormalAction::AttachSelected,
    },
    NormalBinding {
        label: "n",
        desc: "new",
        keys: &[KeyCode::Char('n'), KeyCode::Char('N')],
        action: NormalAction::NewSession,
    },
    NormalBinding {
        label: "d",
        desc: "kill",
        keys: &[KeyCode::Char('d'), KeyCode::Char('D'), KeyCode::Char('x'), KeyCode::Char('X')],
        action: NormalAction::KillSelected,
    },
    NormalBinding {
        label: "q",
        desc: "quit",
        keys: &[KeyCode::Char('q'), KeyCode::Char('Q'), KeyCode::Esc],
        action: NormalAction::Quit,
    },
];

/// Modal-mode footer hints. Display-only: dispatch for these modes
/// lives inline in update.rs because their accepted input set isn't
/// a finite list of keys (CreateInput accepts any printable char;
/// the confirm modes accept `{y, Y}` + `{n, N, Enter, Esc}` with
/// other keys ignored).
pub const CREATE_HINTS: &[(&str, &str)] = &[("ret", "create"), ("esc", "cancel")];

/// Shared by ConfirmKill and ConfirmForce. Split into per-mode
/// constants if the two ever need to diverge.
pub const CONFIRM_HINTS: &[(&str, &str)] = &[("y/N", "")];

/// Whether this keypress should dispatch to a binding / action.
///
/// The whitelist is "modifiers are a subset of `{SHIFT}`". Chord
/// keys (Ctrl / Alt / Super / Hyper / Meta) are filtered out — they
/// shouldn't accidentally trigger plain bindings (e.g. Ctrl-D should
/// not fire `d`'s kill action). Ctrl-C is handled as a special-case
/// global quit in `update`'s key handler, earlier in the dispatch.
///
/// SHIFT is allowed because terminals disagree on where Shift shows
/// up: some fold it into the KeyCode (Shift-j → `Char('J')`,
/// modifiers = NONE), while enhanced protocols report it separately
/// (`Char('J')`, modifiers = SHIFT). Accepting both shapes means
/// Shift-letter variants fire regardless of terminal.
pub fn is_dispatchable(key: &KeyEvent) -> bool {
    (key.modifiers - KeyModifiers::SHIFT).is_empty()
}

/// Look up which NormalAction (if any) this KeyEvent triggers.
///
/// Policy:
///   - Chord keys are filtered via `is_dispatchable` — Ctrl-D, Alt-J, etc.
///     return None even when the underlying char is in the table.
///   - Case folding is explicit in the binding table (see the doc on
///     `NORMAL_BINDINGS`): nothing is folded at lookup time.
pub fn normal_action(key: &KeyEvent) -> Option<NormalAction> {
    if !is_dispatchable(key) {
        return None;
    }
    NORMAL_BINDINGS.iter().find(|b| b.keys.contains(&key.code)).map(|b| b.action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Guard against drift: if a new binding accidentally re-uses a
    /// key already claimed by an existing binding, `normal_action`'s
    /// linear scan would silently dispatch to whichever comes first
    /// and the new binding would never fire. Catch that at test time.
    /// Also catches duplicates within a single binding's `keys` array.
    #[test]
    fn no_key_bound_to_multiple_actions() {
        let mut claimed: HashMap<KeyCode, &'static str> = HashMap::new();
        for b in NORMAL_BINDINGS {
            for &k in b.keys {
                if let Some(existing) = claimed.insert(k, b.label) {
                    panic!(
                        "key {:?} is bound by both [{}] and [{}] — \
                         NORMAL_BINDINGS entries must have disjoint keys",
                        k, existing, b.label,
                    );
                }
            }
        }
    }
}
