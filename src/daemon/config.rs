use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct Config {
    /// norc makes it so that new shells do not load rc files
    /// when they spawn. Only works with bash.
    pub norc: Option<bool>,
    /// shell overrides the user's default shell
    pub shell: Option<String>,
    /// a table of environment variables to inject into the
    /// initial shell
    pub env: Option<HashMap<String, String>>,
    /// a list of environment variables to take from the shell
    /// where `shpool attach` is run. Vars in this list but not
    /// in the clients environment are left untouched.
    /// Overrides `env` vars if they collide. By default
    /// vec!["DISPLAY", "KRB5CCNAME", "SSH_ASKPASS", "SSH_AUTH_SOCK",
    ///      "SSH_AGENT_PID", "SSH_CONNECTION", "WINDOWID", "XAUTHORITY"]
    pub client_env: Option<Vec<String>>,
    /// Disable the tty echo flag for spawned subshells.
    /// You likely don't want to set this, but if you
    /// plan on interacting programatically with the
    /// shells it can make the output easier to parse.
    pub noecho: Option<bool>,
    /// A duration, in milliseconds, that the shpool
    /// daemon should wait for the handshake performed
    /// by the two component threads of the ssh plugin
    /// to complete. 30 seconds by default.
    pub ssh_handshake_timeout_ms: Option<u64>,
}
