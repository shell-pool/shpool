use std::collections::HashMap;

use serde_derive::Deserialize;

#[derive(Deserialize, Default)]
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
    /// simlink to the socket and set SSH_AUTH_SOCK to that simlink
    /// in the shpool session's environment. shpool uses a simlink
    /// in this way rather than directly injecting the SSH_AUTH_SOCK
    /// value into the shell session's environment so that we
    /// can handle it if the value of SSH_AUTH_SOCK changes across
    /// reconnects. Plumbing the SSH_AUTH_SOCK through in this way
    /// is needed to allow users to communicate back to the ssh-agent
    /// running on their client machine in order to do stuff like
    /// use hardware security keys.
    pub nosimlink_ssh_auth_sock: Option<bool>,
    /// shell overrides the user's default shell
    pub shell: Option<String>,
    /// a table of environment variables to inject into the
    /// initial shell
    pub env: Option<HashMap<String, String>>,
    /// a list of environment variables to take from the shell
    /// where `shpool attach` is run. Vars in this list but not
    /// in the clients environment are left untouched.
    /// Overrides `env` vars if they collide. Only applies on
    /// first attach to the environment.
    pub client_env: Option<Vec<String>>,
    /// A duration, in milliseconds, that the shpool
    /// daemon should wait for the handshake performed
    /// by the two component threads of the ssh plugin
    /// to complete. 30 seconds by default.
    pub ssh_handshake_timeout_ms: Option<u64>,
}
