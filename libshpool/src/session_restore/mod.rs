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

pub trait SessionSpool {
    /// Resizes the internal representation to new tty size.
    fn resize(&mut self, size: TtySize);

    /// Gets the current content as a byte buffer.
    fn current_contents(&self) -> Vec<u8>;

    /// Process bytes from pty master.
    fn process(&mut self, bytes: &[u8]);
}

/// A spool that doesn't do anything.
pub struct NullSpool;
impl SessionSpool for NullSpool {
    fn resize(&mut self, _: TtySize) {}

    fn current_contents(&self) -> Vec<u8> {
        vec![]
    }

    fn process(&mut self, _: &[u8]) {}
}

/// A spool that restores the last screenful of content using shpool_vt100.
pub struct Vt100Screen {
    parser: shpool_vt100::Parser,
}

impl SessionSpool for Vt100Screen {
    fn resize(&mut self, size: TtySize) {
        // TODO(pfyu): why u16::MAX? shouldn't this be vterm_width?
        self.parser.screen_mut().set_size(size.rows, u16::MAX);
    }

    fn current_contents(&self) -> Vec<u8> {
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
}

impl SessionSpool for Vt100Lines {
    fn resize(&mut self, size: TtySize) {
        // TODO(pfyu): why u16::MAX? shouldn't this be vterm_width?
        self.parser.screen_mut().set_size(size.rows, u16::MAX);
    }

    fn current_contents(&self) -> Vec<u8> {
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
    mode: &SessionRestoreMode,
    size: &TtySize,
    scrollback_lines: usize,
) -> Box<dyn SessionSpool + 'static> {
    let vterm_width = {
        let config = config.get();
        config.vt100_output_spool_width.unwrap_or(VTERM_WIDTH)
    };

    match mode {
        SessionRestoreMode::Simple => Box::new(NullSpool),
        SessionRestoreMode::Screen => Box::new(Vt100Screen {
            parser: shpool_vt100::Parser::new(size.rows, vterm_width, scrollback_lines),
        }),
        SessionRestoreMode::Lines(nlines) => Box::new(Vt100Lines {
            parser: shpool_vt100::Parser::new(size.rows, vterm_width, scrollback_lines),
            nlines: *nlines,
        }),
    }
}
