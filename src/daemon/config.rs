use std::collections::HashMap;

use serde_derive::Deserialize;

use super::keybindings;

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

    /// shell overrides the user's default shell
    pub shell: Option<String>,

    /// a table of environment variables to inject into the
    /// initial shell
    pub env: Option<HashMap<String, String>>,

    /// The initial path to spawn shell processes with. By default
    /// `/usr/bin:/bin:/usr/sbin:/sbin` (copying openssh).
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
    #[default]
    Simple,
    /// Emit enough data from the output spool to restore the screen
    /// full of text which would have been present on the screen if
    /// the connection never dropped. If a command drops while generating
    /// output, it will restore a screen showing the most recent output
    /// rather than the screen visible right before disconnect.
    Screen,
    /// Emit enough output data to restore the last n lines of
    /// history from the output spool.
    Lines(usize),
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
