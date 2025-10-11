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

use std::io::{Read, Write};

use anyhow::{anyhow, Context};
use tracing::{debug, info, instrument, warn};

use crate::{
    consts::{SENTINEL_FLAG_VAR, STARTUP_SENTINEL},
    daemon::trie::{Trie, TrieCursor},
};

#[derive(Debug, Clone)]
enum KnownShell {
    Bash,
    Zsh,
    Fish,
}

/// Inject the given prefix into the given shell subprocess, using
/// the shell path in `shell` to decide the right way to go about
/// injecting the prefix.
///
/// If the prefix is blank, this is a noop.
#[instrument(skip_all)]
pub fn maybe_inject_prefix(
    pty_master: &mut shpool_pty::fork::Fork,
    prompt_prefix: &str,
    session_name: &str,
) -> anyhow::Result<()> {
    let shell_pid = pty_master.child_pid().ok_or(anyhow!("no child pid"))?;
    // scan for the startup sentinel so we know it is safe to sniff the shell
    let mut pty_master = pty_master.is_parent().context("expected parent")?;
    wait_for_startup(&mut pty_master)?;

    let shell_type = sniff_shell(shell_pid);
    debug!("sniffed shell type: {:?}", shell_type);

    // now actually inject the prompt
    let prompt_prefix = prompt_prefix.replace("$SHPOOL_SESSION_NAME", session_name);

    let mut script = match (prompt_prefix.as_str(), shell_type) {
        (_, Ok(KnownShell::Bash)) => format!(
            r#"
            if [[ -z "${{PROMPT_COMMAND+x}}" ]]; then
               PS1="{prompt_prefix}${{PS1}}"
            else
               SHPOOL__OLD_PROMPT_COMMAND=("${{PROMPT_COMMAND[@]}}")
               SHPOOL__OLD_PS1="${{PS1}}"
               function __shpool__prompt_command() {{
                  PS1="${{SHPOOL__OLD_PS1}}"
                  for prompt_hook in "${{SHPOOL__OLD_PROMPT_COMMAND[@]}}"
                  do
                    eval "${{prompt_hook}}"
                  done
                  PS1="{prompt_prefix}${{PS1}}"
               }}
               PROMPT_COMMAND=__shpool__prompt_command
            fi
        "#
        ),
        (_, Ok(KnownShell::Zsh)) => format!(
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
        ),
        (_, Ok(KnownShell::Fish)) => format!(
            r#"
            functions --copy fish_prompt shpool__old_prompt
            function fish_prompt; echo -n "{prompt_prefix}"; shpool__old_prompt; end
        "#
        ),
        (_, Err(e)) => {
            warn!("could not sniff shell: {}", e);

            // not the end of the world, we will just not inject a prompt prefix
            String::new()
        }
    };

    // With this magic env var set, `shpool daemon` will just
    // print the prompt sentinel and immediately exit. We do
    // this rather than `echo $PROMPT_SENTINEL` because different
    // shells have subtly different echo behavior which makes it
    // hard to make the scanner work right.
    let exe_path = std::env::current_exe()
        .context("getting current exe path")?
        .to_string_lossy()
        .to_string();
    let sentinel_cmd = format!("\n {}=prompt {} daemon\n", SENTINEL_FLAG_VAR, exe_path);
    script.push_str(sentinel_cmd.as_str());

    debug!("injecting prefix script '{}'", script);
    pty_master.write_all(script.as_bytes()).context("running prefix script")?;

    Ok(())
}

#[instrument(skip_all)]
fn wait_for_startup(pty_master: &mut shpool_pty::fork::Master) -> anyhow::Result<()> {
    let mut startup_sentinel_scanner = SentinelScanner::new(STARTUP_SENTINEL);
    let exe_path = std::env::current_exe()
        .context("getting current exe path")?
        .to_string_lossy()
        .to_string();
    let startup_sentinel_cmd = format!("\n {}=startup {} daemon\n", SENTINEL_FLAG_VAR, exe_path);

    pty_master
        .write_all(startup_sentinel_cmd.as_bytes())
        .context("running startup sentinel script")?;

    let mut buf: [u8; 2048] = [0; 2048];
    loop {
        let len = pty_master.read(&mut buf).context("reading chunk to scan for startup")?;
        if len == 0 {
            continue;
        }
        let buf = &buf[..len];
        debug!("buf='{}'", String::from_utf8_lossy(buf));
        for byte in buf.iter() {
            if startup_sentinel_scanner.transition(*byte) {
                // This might drop trailing data from the chunk we just read, but
                // it should be fine since we are about to inject the prompt setup
                // stuff anyway, and shell.rs will scan for the prompt setup sentinel
                // in order to handle the smooth handoff.
                return Ok(());
            }
        }
    }
}

/// Determine the shell process running under the given pid by examining
/// `/proc/<pid>/exe`.
#[instrument(skip_all)]
fn sniff_shell(pid: libc::pid_t) -> anyhow::Result<KnownShell> {
    let shell_proc_name =
        libproc::proc_pid::name(pid).map_err(|e| anyhow!("determining subproc name: {:?}", e))?;
    info!("shell_proc_name: {}", shell_proc_name);

    if shell_proc_name.ends_with("bash") {
        Ok(KnownShell::Bash)
    } else if shell_proc_name.ends_with("zsh") {
        Ok(KnownShell::Zsh)
    } else if shell_proc_name.ends_with("fish") {
        Ok(KnownShell::Fish)
    } else {
        Err(anyhow!("unknown shell: {:?}", shell_proc_name))
    }
}

/// A trie for scanning through shell output to look for the sentinel.
pub struct SentinelScanner {
    scanner: Trie<u8, (), Vec<Option<usize>>>,
    cursor: TrieCursor,
    num_matches: usize,
}

impl SentinelScanner {
    /// Create a new sentinel scanner.
    pub fn new(sentinel: &str) -> Self {
        let mut scanner = Trie::new();
        scanner.insert(sentinel.bytes(), ());

        SentinelScanner { scanner, cursor: TrieCursor::Start, num_matches: 0 }
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
