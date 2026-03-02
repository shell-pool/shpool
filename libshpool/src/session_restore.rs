// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use shpool_protocol::TtySize;
use tracing::info;

use crate::config::{self, SessionRestoreMode};

// To prevent data getting dropped, we set this to be large, but we don't want
// to use u16::MAX, since the vt100 crate eagerly fills in its rows, and doing
// so is very memory intensive. The right fix is to get the vt100 crate to
// lazily initialize its rows, but that is likely a bunch of work.
const VTERM_WIDTH: u16 = 1024;

/// Some session shpool specific config getters
trait ConfigExt {
    /// Effective vterm width.
    ///
    /// See also `VTERM_WIDTH`.
    fn vterm_width(&self) -> u16;
}

impl ConfigExt for config::Manager {
    fn vterm_width(&self) -> u16 {
        let config = self.get();
        config.vt100_output_spool_width.unwrap_or(VTERM_WIDTH)
    }
}

pub trait SessionSpool {
    /// Resizes the internal representation to new tty size.
    fn resize(&mut self, size: TtySize);

    /// Gets a byte sequence to restore the on-screen session content.
    ///
    /// The returned sequence is expected to be able to restore the screen
    /// content regardless of any prior screen state. It thus mostly likely
    /// includes some terminal control codes to reset the screen from any
    /// state back to a known good state.
    ///
    /// Note that what exactly is restored is determined by the implementation,
    /// and thus can vary from do nothing to a few lines to a full screen,
    /// etc.
    fn restore_buffer(&self) -> Vec<u8>;

    /// Process bytes from pty master.
    fn process(&mut self, bytes: &[u8]);
}

/// A spool that doesn't do anything.
pub struct NullSpool;
impl SessionSpool for NullSpool {
    fn resize(&mut self, _: TtySize) {}

    fn restore_buffer(&self) -> Vec<u8> {
        info!("generating null restore buf");
        vec![]
    }

    fn process(&mut self, _: &[u8]) {}
}

/// A spool that restores the last screenful of content using shpool_vt100.
pub struct Vt100Screen {
    parser: shpool_vt100::Parser,
    /// Other options will be read dynamically from config.
    config: config::Manager,
}

impl SessionSpool for Vt100Screen {
    fn resize(&mut self, size: TtySize) {
        self.parser.screen_mut().set_size(size.rows, self.config.vterm_width());
    }

    fn restore_buffer(&self) -> Vec<u8> {
        let (rows, cols) = self.parser.screen().size();
        info!("computing screen restore buf with (rows={}, cols={})", rows, cols);
        self.parser.screen().contents_formatted()
    }

    fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes)
    }
}

/// A spool that restores the last n lines of content using shpool_vt100.
pub struct Vt100Lines {
    parser: shpool_vt100::Parser,
    /// How many lines to restore
    nlines: u16,
    /// Other options will be read dynamically from config.
    config: config::Manager,
}

impl SessionSpool for Vt100Lines {
    fn resize(&mut self, size: TtySize) {
        self.parser.screen_mut().set_size(size.rows, self.config.vterm_width());
    }

    fn restore_buffer(&self) -> Vec<u8> {
        let (rows, cols) = self.parser.screen().size();
        info!("computing lines({}) restore buf with (rows={}, cols={})", self.nlines, rows, cols);
        self.parser.screen().last_n_rows_contents_formatted(self.nlines)
    }

    fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes)
    }
}

/// Creates a spool given a `mode`.
pub fn new(
    config: config::Manager,
    size: &TtySize,
    scrollback_lines: usize,
) -> Box<dyn SessionSpool + 'static> {
    let mode = config.get().session_restore_mode.clone().unwrap_or_default();
    let vterm_width = config.vterm_width();
    match mode {
        SessionRestoreMode::Simple => Box::new(NullSpool),
        SessionRestoreMode::Screen => Box::new(Vt100Screen {
            parser: shpool_vt100::Parser::new(size.rows, vterm_width, scrollback_lines),
            config,
        }),
        SessionRestoreMode::Lines(nlines) => Box::new(Vt100Lines {
            parser: shpool_vt100::Parser::new(size.rows, vterm_width, scrollback_lines),
            nlines,
            config,
        }),
    }
}
