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

// This file contains the logic for injecting the `prompt_annotation`
// config option into a user's prompt for known shells.

use std::io::Write;

use anyhow::Context;
use tracing::{debug, instrument, warn};

use crate::{
    consts::{PROMPT_SENTINEL, PROMPT_SENTINEL_FLAG_VAR},
    daemon::trie::{Trie, TrieCursor},
};

/// Inject the given prefix into the given shell subprocess, using
/// the shell path in `shell` to decide the right way to go about
/// injecting the prefix.
#[instrument(skip_all)]
pub fn inject_prefix(
    pty_master: &mut shpool_pty::fork::Fork,
    shell: Option<&str>,
    prompt_prefix: &str,
    session_name: &str,
) -> anyhow::Result<()> {
    let prompt_prefix = prompt_prefix.replace("$SHPOOL_SESSION_NAME", session_name);
    let mut script = if prompt_prefix.is_empty() {
        // We still need to emit the sentinel for consistency
        // even when we don't have a prompt prefix to inject.
        String::new()
    } else if shell.map(|s| s.ends_with("bash")).unwrap_or(false) {
        format!(
            r#"
            if [[ -z "${{PROMPT_COMMAND+x}}" ]]; then
               PS1="{prompt_prefix}${{PS1}}"
            else
               SHPOOL__OLD_PROMPT_COMMAND="${{PROMPT_COMMAND}}"
               SHPOOL__OLD_PS1="${{PS1}}"
               function __shpool__prompt_command() {{
                  PS1="${{SHPOOL__OLD_PS1}}"
                  for prompt_hook in ${{SHPOOL__OLD_PROMPT_COMMAND}}
                  do
                    ${{prompt_hook}}
                  done
                  PS1="{prompt_prefix}${{PS1}}"
               }}
               PROMPT_COMMAND=__shpool__prompt_command
            fi
"#
        )
    } else if shell.map(|s| s.ends_with("zsh")).unwrap_or(false) {
        format!(
            r#"
            typeset -a precmd_functions
            SHPOOL__OLD_PROMPT="${{PROMPT}}"
            function __shpool__reset_rprompt() {{
                PROMPT="${{SHPOOL__OLD_PROMPT}}"
            }}
            precmd_functions[1,0]=(__shpool__reset_rprompt)
            function __shpool__prompt_command() {{
               PROMPT="{prompt_prefix}${{PROMPT}}"
            }}
            precmd_functions+=(__shpool__prompt_command)
"#
        )
    } else if shell.map(|s| s.ends_with("fish")).unwrap_or(false) {
        format!(
            r#"
            functions --copy fish_prompt shpool__old_prompt
            function fish_prompt; echo -n "{prompt_prefix}"; shpool__old_prompt; end
"#
        )
    } else {
        warn!("don't know how to inject a prefix for shell '{:?}'", shell);
        // We still need to emit the sentinel for consistency
        // even when we don't have a prompt prefix to inject.
        String::new()
    };

    // With this magic env var set, `shpool daemon` will just
    // print the prompt sentinel and immediately exit. We do
    // this rather than `echo $PROMPT_SENTINEL` because different
    // shells have subtly different echo behavior which makes it
    // hard to make the scanner work right.
    // TODO(julien): this will probably not work on mac
    let sentinel_cmd =
        format!("\n{}=yes /proc/{}/exe daemon\n", PROMPT_SENTINEL_FLAG_VAR, std::process::id());
    script.push_str(sentinel_cmd.as_str());

    let mut pty_master = pty_master.is_parent().context("expected parent")?;
    pty_master.write_all(script.as_bytes()).context("running prefix script")?;

    Ok(())
}

/// A trie for scanning through shell output to look for the sentinel.
pub struct SentinalScanner {
    scanner: Trie<u8, (), Vec<Option<usize>>>,
    cursor: TrieCursor,
    num_matches: usize,
}

impl SentinalScanner {
    /// Create a new sentinel scanner.
    pub fn new() -> Self {
        let mut scanner = Trie::new();
        scanner.insert(PROMPT_SENTINEL.bytes(), ());

        SentinalScanner { scanner, cursor: TrieCursor::Start, num_matches: 0 }
    }

    // Pump the given byte through the scanner, returning true if the underlying
    // shell has finished printing the sentinel value.
    pub fn transition(&mut self, byte: u8) -> bool {
        self.cursor = self.scanner.advance(self.cursor, byte);
        match self.cursor {
            TrieCursor::NoMatch => {
                self.cursor = TrieCursor::Start;
                false
            }
            TrieCursor::Match { is_partial, .. } if !is_partial => {
                self.cursor = TrieCursor::Start;
                self.num_matches += 1;
                debug!("got prompt sentinel match #{}", self.num_matches);
                self.num_matches == 1
            }
            _ => false,
        }
    }
}
