//! `with_tui_suspended` — run some code with the TUI's terminal state
//! torn down (raw mode off, alt screen left, cursor shown) and
//! automatically restore on return.
//!
//! Used for spawning `shpool attach`: the child needs a clean tty to
//! take over, and we need to make sure we come back to our alt
//! screen even if `f` returned an error.

use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// Tear the TUI down, run `f`, put the TUI back up. Restores on both
/// success and error-return paths of `f`.
pub fn with_tui_suspended<F, R>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    f: F,
) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    // --- leave TUI mode ---
    // Order matters: leave alt screen BEFORE disabling raw mode, so
    // the alt-screen escape is sent while raw mode is still active
    // (some terminals buffer differently otherwise). Show the cursor
    // last; that's cosmetic.
    //
    // After leaving the alt screen we're back on the primary screen,
    // which still holds whatever was there before the TUI opened
    // (shell prompts, command history, etc.). We clear it before
    // handing control to `f` so the child (typically `shpool
    // attach`) starts on a blank viewport. `Clear::All` wipes the
    // visible screen; scrollback is preserved. `MoveTo(0,0)` homes
    // the cursor so the child's first output lands at the top-left.
    execute!(io::stdout(), LeaveAlternateScreen, Clear(ClearType::All), MoveTo(0, 0), Show,)
        .context("leaving alt screen")?;
    disable_raw_mode().context("disabling raw mode")?;

    // --- run the caller's thing ---
    // We capture the result so we can still restore the terminal even
    // if `f` returned Err. We do NOT use `?` here because that would
    // early-return and skip the restore.
    let result = f();

    // --- re-enter TUI mode ---
    // Same ordering in reverse: raw mode first, then alt screen.
    enable_raw_mode().context("re-enabling raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, Hide).context("re-entering alt screen")?;

    // The child may have drawn arbitrary content onto the main screen;
    // telling ratatui to forget its previous buffer forces a full
    // redraw of the TUI on the next `.draw` call.
    terminal.clear().context("clearing terminal after resume")?;

    result
}
