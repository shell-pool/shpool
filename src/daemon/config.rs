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

    /// A duration, in milliseconds, that the shpool
    /// daemon should wait for the handshake performed
    /// by the two component threads of the ssh plugin
    /// to complete. 30 seconds by default.
    pub ssh_handshake_timeout_ms: Option<u64>,

    /// The initial path to spawn shell processes with. By default
    /// `/usr/bin:/bin:/usr/sbin:/sbin` (copying openssh).
    pub initial_path: Option<String>,

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
