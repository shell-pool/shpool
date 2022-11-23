use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
pub struct Config {
    /// norc makes it so that new shells do not load rc files when they
    /// spawn. Only works with bash.
    pub norc: Option<bool>,
    /// shell overrides the user's default shell
    pub shell: Option<String>,
    /// a table of environment variables to inject into the initial shell
    pub env: Option<HashMap<String, String>>,
    /// Disable the tty echo flag for spawned subshells. You likely don't
    /// want to set this, but if you plan on interacting programatically
    /// with the shells it can make the output easier to parse.
    pub noecho: Option<bool>,
}
