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

use std::{collections::HashMap, process::Command};

use anyhow::{anyhow, Context};

use super::trie::{Trie, TrieCursor};

/// Contains metadata used to create new matchers.
#[derive(Debug)]
pub struct MatcherFactory {
    /// A table mapping TERM values to the output that the `clear`
    /// command produces when run with that TERM value. Really,
    /// it would be nice to do this with some judicious queries
    /// to to termini::TermInfo, but unfortunately I can't figure
    /// out the right query to get the second control code that
    /// `clear` emits. The first is `termini::StringCapability::ClearScreen`.
    clear_codes: HashMap<String, Vec<u8>>,
}

impl MatcherFactory {
    /// Create a new factory.
    pub fn new() -> Self {
        MatcherFactory { clear_codes: HashMap::new() }
    }

    /// Create a new matcher for the given TERM value.
    pub fn new_matcher(&mut self, term: &str) -> anyhow::Result<Matcher> {
        let clear_code = match self.clear_codes.get(term) {
            Some(c) => c,
            None => {
                let code = Self::clear_code_for_term(term)?;
                self.clear_codes.insert(String::from(term), code);
                self.clear_codes.get(term).unwrap()
            }
        };

        Matcher::new(clear_code)
    }

    fn clear_code_for_term(term: &str) -> anyhow::Result<Vec<u8>> {
        let output = Command::new("clear")
            .env("TERM", term)
            .output()
            .context("execing clear to get output")?;
        if !output.status.success() {
            return Err(anyhow!("bad clear exit status: {:?}", output.status));
        }
        if !output.stderr.is_empty() {
            return Err(anyhow!(
                "got clear stderr (want none): {}",
                String::from_utf8_lossy(output.stderr.as_slice())
            ));
        }
        if output.stdout.is_empty() {
            return Err(anyhow!("got empty clear stdout"));
        }
        Ok(output.stdout)
    }
}

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
    fn new(clear_code_bytes: &[u8]) -> anyhow::Result<Self> {
        let raw_bindings = vec![
            // We need to scan for the clear code that gets emitted by the prompt prefix
            // shell injection code so that we can make sure that the message of the day
            // won't get clobbered immediately.
            (clear_code_bytes, Code::ClearScreen),
        ];
        let mut codes = Trie::new();
        for (raw_bytes, code) in raw_bindings.into_iter() {
            codes.insert(raw_bytes.iter().copied(), code);
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
