//! `shpool tui` — interactive session manager.

mod command;
mod event;
mod keymap;
mod model;
mod suspend;
mod update;
mod view;

use std::path::PathBuf;

use anyhow::Result;

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
    let _ = (socket, config_file, log_file, verbose);
    todo!()
}
