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

use anyhow::{anyhow, Context};
use tracing::instrument;

/// Inject the given prefix into the given shell subprocess, using
/// the shell path in `shell` to decide the right way to go about
/// injecting the prefix.
#[instrument(skip_all)]
pub fn inject_prefix(
    pty_master: &mut shpool_pty::fork::Fork,
    shell: &str,
    prompt_prefix: &str,
    session_name: &str,
) -> anyhow::Result<()> {
    let prompt_prefix = prompt_prefix.replace("$SHPOOL_SESSION_NAME", session_name);
    if shell.ends_with("bash") {
        let script = format!(
            r#"
            if [[ -z "${{PROMPT_COMMAND+x}}" ]]; then
               PS1="{prompt_prefix}${{PS1}}"
            else
               SHPOOL__OLD_PROMPT_COMMAND="${{PROMPT_COMMAND}}"
               SHPOOL__OLD_PS1="${{PS1}}"
               function __shpool__prompt_command() {{
                  PS1="${{SHPOOL__OLD_PS1}}"
                  ${{SHPOOL__OLD_PROMPT_COMMAND}}
                  PS1="{prompt_prefix}${{PS1}}"
               }}
               PROMPT_COMMAND=__shpool__prompt_command
            fi
            clear
"#
        );
        let mut pty_master = pty_master.is_parent().context("expected parent")?;
        pty_master.write_all(script.as_bytes()).context("running prefix script")?;
    } else if shell.ends_with("zsh") {
        let script = format!(
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
            clear
"#
        );
        let mut pty_master = pty_master.is_parent().context("expected parent")?;
        pty_master.write_all(script.as_bytes()).context("running prefix script")?;
    } else {
        return Err(anyhow!("don't know how to inject a prefix for shell '{}'", shell));
    }

    Ok(())
}
