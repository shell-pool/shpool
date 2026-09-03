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

use std::{
    io::{Read, Write},
    time,
};

use anyhow::{anyhow, Context};
use nix::{poll, poll::PollFlags};
use tracing::{debug, error, info, instrument, warn};

use crate::{
    consts::{SENTINEL_FLAG_VAR, STARTUP_SENTINEL},
    daemon::trie::{Trie, TrieCursor},
    exe, test_hooks,
};

// We don't need an agressive poll cadence because the normal case is
// that we exit the startup sentinal loop after some data comes in and
// we scan the sentinal successfully. We only need to wake up every so
// often to check if we've hit our timeout, which is long.
const SENTINEL_POLL_MS: u16 = 500;

// Even the most sluggish dotfile setup ought to be done within
// 90 seconds.
const SENTINEL_POLL_TIMEOUT: time::Duration = time::Duration::from_secs(90);

#[derive(Debug, Clone)]
enum KnownShell {
    Bash,
    Zsh,
    Fish,
}

/// Inject the given prefix and startup cmdn into the given shell subprocess,
/// using the shell path in `shell` to decide the right way to go about
/// injecting the prefix.
///
/// If either the prefix or startup cmd are blank, we do nothing for that
/// option.
#[instrument(skip_all)]
pub fn maybe_setup(
    pty_master: &mut shpool_pty::fork::Fork,
    prompt_prefix: &str,
    start_cmd: &str,
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
        (_, Ok(KnownShell::Bash)) => {
            // In Bash 5.1+, PROMPT_COMMAND supports arrays. However, older
            // versions of Bash (such as Bash 3.2, the default system shell on
            // macOS) only execute PROMPT_COMMAND if it is a scalar string;
            // assigning an array causes Bash 3.2 to silently ignore it.
            // We capture any existing hooks (array or scalar) into
            // SHPOOL__OLD_PROMPT_COMMAND, unset PROMPT_COMMAND, and assign
            // PROMPT_COMMAND as a scalar string to ensure universal
            // compatibility.
            format!(
                r#"
           SHPOOL__OLD_PROMPT_COMMAND=("${{PROMPT_COMMAND[@]}}")
           SHPOOL__OLD_PS1="${{PS1}}"
           function __shpool__prompt_command() {{
              local ret=$?
              local env_file="${{SHPOOL_SESSION_DIR}}/forward.env"
              local stamp_file="${{env_file}}.stamp"
              if [ -n "${{SHPOOL_SESSION_DIR}}" ] && [ -f "${{env_file}}" ]; then
                if [ ! -f "${{stamp_file}}" ] || [ "${{env_file}}" -nt "${{stamp_file}}" ]; then
                  touch -r "${{env_file}}" "${{stamp_file}}" 2>/dev/null

                  local allexport_was_set=0
                  case "$-" in
                    *a*) allexport_was_set=1 ;;
                  esac
                  set -a
                  . "${{env_file}}"
                  if [ "$allexport_was_set" -eq 0 ] ; then
                    set +a
                  fi
                fi
              fi

              PS1="${{SHPOOL__OLD_PS1}}"
              (exit $ret)
              for prompt_hook in "${{SHPOOL__OLD_PROMPT_COMMAND[@]}}"
              do
                eval "${{prompt_hook}}"
                ret=$?
              done
              PS1="{prompt_prefix}${{PS1}}"
              return $ret
           }}
           unset PROMPT_COMMAND
           PROMPT_COMMAND=__shpool__prompt_command
        "#
            )
        }
        (_, Ok(KnownShell::Zsh)) => format!(
            r#"
            typeset -a precmd_functions
            SHPOOL__OLD_PROMPT="${{PROMPT}}"
            function __shpool__reset_rprompt() {{
                local ret=$?
                local env_file="${{SHPOOL_SESSION_DIR:-}}/forward.env"
                local stamp_file="${{env_file}}.stamp"
                if [ -n "${{SHPOOL_SESSION_DIR:-}}" ] && [ -f "${{env_file}}" ]; then
                  if [ ! -f "${{stamp_file}}" ] || [ "${{env_file}}" -nt "${{stamp_file}}" ]; then
                    touch -r "${{env_file}}" "${{stamp_file}}" 2>/dev/null

                    local allexport_was_set=0
                    case "$-" in
                      *a*) allexport_was_set=1 ;;
                    esac
                    set -a
                    . "${{env_file}}"
                    if [ "$allexport_was_set" -eq 0 ] ; then
                      set +a
                    fi
                  fi
                fi

                PROMPT="${{SHPOOL__OLD_PROMPT}}"
                return $ret
            }}
            precmd_functions[1,0]=(__shpool__reset_rprompt)
            function __shpool__prompt_command() {{
               local ret=$?
               PROMPT="{prompt_prefix}${{PROMPT}}"
               return $ret
            }}
            precmd_functions+=(__shpool__prompt_command)
        "#
        ),
        (_, Ok(KnownShell::Fish)) => {
            // Fish only added the `-nt` (newer-than) binary operator to its
            // builtin `test` in fish 4.0b1. In older fish versions (such as
            // fish 3.x), calling builtin `test -nt` errors with "unexpected
            // argument". To maintain zero-fork prompt evaluation on
            // fish 4+ while remaining compatible with fish 3, we
            // probe for `-nt` support once at injection
            // time and define `__shpool_is_newer` to use the builtin if
            // available, falling back to `command test` (coreutils)
            // otherwise.
            format!(
                r#"
                functions --copy fish_prompt shpool__old_prompt
                function __shpool_set_status; return $argv[1]; end
                set -l __shpool_nt_err (test /dev/null -nt /dev/null 2>&1)
                if test -z "$__shpool_nt_err"
                    function __shpool_is_newer; test $argv[1] -nt $argv[2]; end
                else
                    function __shpool_is_newer; command test $argv[1] -nt $argv[2]; end
                end
                function fish_prompt
                    set -l last_status $status
                    set -l env_file "$SHPOOL_SESSION_DIR/forward.env"
                    set -l stamp_file "$env_file.stamp"
                    if test -n "$SHPOOL_SESSION_DIR"; and test -f "$env_file"
                        if test ! -f "$stamp_file"; or __shpool_is_newer "$env_file" "$stamp_file"
                            touch -r "$env_file" "$stamp_file" 2>/dev/null
                            source "$env_file"
                        end
                    end
                    echo -n "{prompt_prefix}"
                    __shpool_set_status $last_status
                    shpool__old_prompt
                end
            "#
            )
        }
        (_, Err(e)) => {
            warn!("could not sniff shell: {}", e);

            // not the end of the world, we will just not inject a prompt prefix
            String::new()
        }
    };

    if !start_cmd.is_empty() {
        script.push('\n');
        script.push_str(start_cmd);
        script.push('\n');
    }

    // With this magic env var set, `shpool daemon` will just
    // print the prompt sentinel and immediately exit. We do
    // this rather than `echo $PROMPT_SENTINEL` because different
    // shells have subtly different echo behavior which makes it
    // hard to make the scanner work right.
    let exe_path =
        exe::current().context("getting current exe path")?.to_string_lossy().into_owned();
    let sentinel_cmd = format!("\n {}=prompt {} daemon\n", SENTINEL_FLAG_VAR, exe_path);
    script.push_str(sentinel_cmd.as_str());

    debug!("injecting shell startup script '{}'", script);
    pty_master.write_all(script.as_bytes()).context("running prefix script")?;

    Ok(())
}

#[instrument(skip_all)]
fn wait_for_startup(pty_master: &mut shpool_pty::fork::Master) -> anyhow::Result<()> {
    test_hooks::emit("wait-for-startup-enter");
    let mut startup_sentinel_scanner = SentinelScanner::new(STARTUP_SENTINEL);
    let exe_path =
        exe::current().context("getting current exe path")?.to_string_lossy().into_owned();
    let startup_sentinel_cmd = format!("\n {}=startup {} daemon\n", SENTINEL_FLAG_VAR, exe_path);

    pty_master
        .write_all(startup_sentinel_cmd.as_bytes())
        .context("running startup sentinel script")?;

    let watchable_master = pty_master.clone();
    let mut poll_fds = [poll::PollFd::new(
        watchable_master.borrow_fd(),
        PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
    )];

    let deadline = time::Instant::now() + SENTINEL_POLL_TIMEOUT;
    let mut buf: [u8; 2048] = [0; 2048];
    loop {
        if time::Instant::now() > deadline {
            return Err(anyhow!("timed out waiting for shell startup"));
        }
        let nready = match poll::poll(&mut poll_fds, SENTINEL_POLL_MS) {
            Ok(n) => n,
            Err(e) => {
                error!("polling pty master: {:?}", e);
                return Err(e)?;
            }
        };
        if nready == 0 {
            // if timeout
            continue;
        }
        if nready != 1 {
            return Err(anyhow!("sentinal scan: expected exactly 1 ready fd"));
        }

        let len = pty_master.read(&mut buf).context("reading chunk to scan for startup")?;
        if len == 0 {
            return Err(anyhow!("EOF during shell startup"));
        }
        let buf = &buf[..len];
        debug!("buf='{}'", String::from_utf8_lossy(buf));
        for byte in buf.iter() {
            if startup_sentinel_scanner.transition(*byte) {
                // This might drop trailing data from the chunk we just read,
                // but it should be fine since we are about to
                // inject the prompt setup stuff anyway, and
                // shell.rs will scan for the prompt setup sentinel
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
