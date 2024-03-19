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

//! The escape codes module provides an online (trie based) matcher
//! to scan for escape codes we are interested in in the output of
//! the subshell. For the moment, we just use this to scan for
//! the ClearScreen code emitted by the prompt prefix injection shell
//! code. We need to scan for this to avoid a race that can lead to
//! the motd getting clobbered when in dump mode.

use anyhow::{anyhow, Context};

use super::trie::{Trie, TrieCursor};

#[derive(Debug, Clone, Copy)]
pub enum Code {
    ClearScreen,
}

#[derive(Debug)]
pub struct Matcher {
    codes: Trie<u8, Code, Vec<Option<usize>>>,
    codes_cursor: TrieCursor,
}

impl Matcher {
    pub fn new(term_db: &termini::TermInfo) -> anyhow::Result<Self> {
        let clear_code_bytes = match term_db.raw_string_cap(termini::StringCapability::ClearScreen)
        {
            Some(code) => Vec::from(code),
            None => {
                // If we somehow have a wacky terminfo db with no clear code, we fall
                // back on xterm clear since we still need something to scan for.
                let xterm_db =
                    termini::TermInfo::from_name("xterm").context("building fallback xterm db")?;
                let code = xterm_db
                    .raw_string_cap(termini::StringCapability::ClearScreen)
                    .ok_or(anyhow!("no fallback clear screen code"))?;
                Vec::from(code)
            }
        };

        let raw_bindings = vec![
            // We need to scan for the clear code that gets emitted by the prompt prefix
            // shell injection code so that we can make sure that the message of the day
            // won't get clobbered immediately.
            (clear_code_bytes, Code::ClearScreen),
        ];
        let mut codes = Trie::new();
        for (raw_bytes, code) in raw_bindings.into_iter() {
            codes.insert(raw_bytes.into_iter(), code);
        }

        Ok(Matcher { codes, codes_cursor: TrieCursor::Start })
    }

    pub fn transition(&mut self, byte: u8) -> Option<Code> {
        self.codes_cursor = self.codes.advance(self.codes_cursor, byte);
        match self.codes_cursor {
            TrieCursor::NoMatch => {
                self.codes_cursor = TrieCursor::Start;
                None
            }
            TrieCursor::Match { is_partial, .. } if !is_partial => {
                let code = self.codes.get(self.codes_cursor).copied();
                self.codes_cursor = TrieCursor::Start;
                code
            }
            _ => None,
        }
    }
}
