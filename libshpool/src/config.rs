// Copyright 2023 Google LLC
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

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde_derive::Deserialize;
use tracing::{info, instrument};

use super::{daemon::keybindings, user};

#[instrument(skip_all)]
pub fn read_config(config_file: &Option<String>) -> anyhow::Result<Config> {
    let mut config = Config::default();
    if let Some(config_path) = config_file {
        info!("parsing explicitly passed in config ({})", config_path);
        let config_str = fs::read_to_string(config_path).context("reading config toml (1)")?;
        config = toml::from_str(&config_str).context("parsing config file (1)")?;
    } else {
        let user_info = user::info()?;
        let mut config_path = PathBuf::from(user_info.home_dir);
        config_path.push(".config");
        config_path.push("shpool");
        config_path.push("config.toml");
        if config_path.exists() {
            let config_str = fs::read_to_string(config_path).context("reading config toml (2)")?;
            config = toml::from_str(&config_str).context("parsing config file (2)")?;
        }
    }

    Ok(config)
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Config {
    /// norc makes it so that new shells do not load rc files
    /// when they spawn. Only works with bash.
    pub norc: Option<bool>,

    /// Disable the tty echo flag for spawned subshells.
    /// You likely don't want to set this, but if you
    /// plan on interacting programatically with the
    /// shells it can make the output easier to parse.
    pub noecho: Option<bool>,

    /// By default, if there is a SSH_AUTH_SOCK in the environment
    /// where `shpool attach` gets run, shpool will create a
    /// symlink to the socket and set SSH_AUTH_SOCK to that symlink
    /// in the shpool session's environment. shpool uses a symlink
    /// in this way rather than directly injecting the SSH_AUTH_SOCK
    /// value into the shell session's environment so that we
    /// can handle it if the value of SSH_AUTH_SOCK changes across
    /// reconnects. Plumbing the SSH_AUTH_SOCK through in this way
    /// is needed to allow users to communicate back to the ssh-agent
    /// running on their client machine in order to do stuff like
    /// use hardware security keys.
    pub nosymlink_ssh_auth_sock: Option<bool>,

    /// By default, shpool will read /etc/environment and inject the
    /// variables found there into new shells. If this flag is set,
    /// it will avoid doing so.
    pub noread_etc_environment: Option<bool>,

    /// shell overrides the user's default shell
    pub shell: Option<String>,

    /// a table of environment variables to inject into the
    /// initial shell
    pub env: Option<HashMap<String, String>>,

    /// A list of environment variables to forward from the environment
    /// of the initial shell that invoked `shpool attach` to the newly
    /// launched shell. Note that this config option has no impact when
    /// reattaching to an existing shell.
    pub forward_env: Option<Vec<String>>,

    /// The initial path to spawn shell processes with. By default
    /// `/usr/bin:/bin:/usr/sbin:/sbin` (copying openssh). This
    /// value is often overridden by /etc/environment even if you
    /// do set it.
    pub initial_path: Option<String>,

    /// Indicates what shpool should do when it reattaches to an
    /// existing session.
    pub session_restore_mode: Option<SessionRestoreMode>,

    /// The number of lines worth of output to keep in the output
    /// spool which is maintained along side a shell session.
    /// By default, 10000 lines.
    pub output_spool_lines: Option<usize>,

    /// The user supplied keybindings.
    pub keybinding: Option<Vec<Keybinding>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Keybinding {
    /// The keybinding to map to an action. The syntax for these keybindings
    /// is described in src/daemon/keybindings.rs.
    pub binding: String,
    /// The action to perform in response to the keybinding.
    pub action: keybindings::Action,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionRestoreMode {
    /// Just reattach to the pty and issue SIGWINCH to force apps like
    /// vim and emacs to redraw. Don't emit anything from the output
    /// spool at all.
    Simple,
    /// Emit enough data from the output spool to restore the screen
    /// full of text which would have been present on the screen if
    /// the connection never dropped. If a command drops while generating
    /// output, it will restore a screen showing the most recent output
    /// rather than the screen visible right before disconnect.
    #[default]
    Screen,
    /// Emit enough output data to restore the last n lines of
    /// history from the output spool.
    Lines(u16),
}

#[cfg(test)]
mod test {
    use super::*;
    use ntest::timeout;

    #[test]
    #[timeout(30000)]
    fn parse() -> anyhow::Result<()> {
        let cases = vec![
            r#"
            session_restore_mode = "simple"
            "#,
            r#"
            session_restore_mode = { lines = 10 }
            "#,
            r#"
            session_restore_mode = "screen"
            "#,
            r#"
            [[keybinding]]
            binding = "Ctrl-q a"
            action = "detach"
            "#,
        ];

        for case in cases.into_iter() {
            let _: Config = toml::from_str(case)?;
        }

        Ok(())
    }
}
