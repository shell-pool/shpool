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

use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use anyhow::{Context as _, Result};
use directories::ProjectDirs;
use serde_derive::Deserialize;
use tracing::{info, warn};

use crate::{config_watcher::ConfigWatcher, daemon::keybindings, test_hooks};

/// Exposes the shpool config file, watching for file updates
/// so that the user does not need to restart the daemon when
/// they edit their config.
///
/// Users should never cache the config value directly and always
/// access the config through the manager. The config may change
/// at any time, so any cached config value could become stale.
#[derive(Clone)]
pub struct Manager {
    /// The config value.
    config: Arc<RwLock<Config>>,
    _watcher: Arc<ConfigWatcher>,
}

impl Manager {
    /// Create a new config manager.
    ///
    /// Unless given as the `config_file` argument, config files are read from
    /// the following paths in the reverse priority order:
    ///
    /// - System level config: /etc/shpool/config.toml
    /// - User level config: $XDG_CONFIG_HOME/shpool/config.toml or
    ///   $HOME/.config/shpool/config.toml if $XDG_CONFIG_HOME is not set
    ///
    /// For each top level field, values read later will overrides those read
    /// eariler. The exact merging strategy is as defined in
    /// `Config::merge`.
    pub fn new(config_file: Option<&str>) -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "shpool").context("getting ProjectDirs")?;

        let config_files = match config_file {
            None => {
                vec![
                    Cow::from(Path::new("/etc/shpool/config.toml")),
                    Cow::from(dirs.config_dir().join("config.toml")),
                ]
            }
            Some(config_file) => {
                info!("parsing explicitly passed in config ({})", config_file);
                vec![Cow::from(Path::new(config_file))]
            }
        };

        let config = Self::load(&config_files).context("loading initial config")?;
        info!("starting with config: {:?}", config);
        let config = Arc::new(RwLock::new(config));

        let watcher = {
            let config = config.clone();
            // create a owned version of config_files to move to the watcher thread.
            let config_files: Vec<_> = config_files.iter().map(|f| f.to_path_buf()).collect();
            ConfigWatcher::new(move || {
                info!("reloading config");
                let mut config = config.write().unwrap();
                match Self::load(&config_files) {
                    Ok(c) => {
                        info!("new config: {:?}", c);
                        *config = c;
                    }
                    Err(err) => warn!("error loading config file: {:?}", err),
                }
                test_hooks::emit("daemon-reload-config");
            })
            .context("building watcher")?
        };
        for path in config_files {
            watcher.watch(path).context("registering config file for watching")?;
        }
        let manager = Manager { config, _watcher: Arc::new(watcher) };

        Ok(manager)
    }

    /// Get the current config value.
    pub fn get(&self) -> RwLockReadGuard<'_, Config> {
        self.config.read().unwrap()
    }

    /// Load config by merging configurations from a list of Paths.
    ///
    /// Paths come later in the list takes higher priority.
    /// Merge strategy is as defined in `Config::merge`.
    fn load<T>(config_files: T) -> Result<Config>
    where
        T: IntoIterator,
        T::Item: AsRef<Path>,
    {
        let mut config = Config::default();
        for path in config_files {
            let path = path.as_ref();
            info!("loading config from {:?}", path);
            let config_str = match fs::read_to_string(path) {
                Err(e) => {
                    warn!("skip reading config file {}: {:?}", path.display(), e);
                    continue;
                }
                Ok(s) => s,
            };
            let new_config: Config = match toml::from_str(&config_str) {
                Err(e) => {
                    warn!("error parsing config file: {:?}", e);
                    return Err(e).with_context(|| {
                        format!("parsing config toml {}", path.to_string_lossy())
                    });
                }
                Ok(c) => c,
            };
            config = new_config.merge(config);
        }
        Ok(config)
    }
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let config = self.config.read().unwrap();
        write!(f, "{:?}", config)?;

        Ok(())
    }
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

    /// By default, shpool will check for a running daemon, and if
    /// one is not found, automatically spawn a daemon in the background.
    /// With this option set, it will not do this by default.
    /// (though this value will be overridden by the -d/--daemonize
    /// or -D/--no-daemonize flags).
    pub nodaemonize: Option<bool>,

    /// If set, autodamonization will not timeout when waiting for the
    /// daemon to come up and will instead spin forever.
    pub nodaemonize_timeout: Option<bool>,

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

    /// The width to use when storing session restoration lines via
    /// the vt100 in-memory terminal engine (for the moment, the only
    /// supported engine). vt100 allocates memory for the full width
    /// for each line, leading to a lot of memory overhead, so playing
    /// with this setting can be useful for tuning shpool's memory
    /// usage. Eventually, this option will be deprecated once
    /// the vt100 engine has been replaced.
    pub vt100_output_spool_width: Option<u16>,

    /// The user supplied keybindings.
    pub keybinding: Option<Vec<Keybinding>>,

    /// A prefix to inject into the prompt of freshly spawned shells.
    /// The prefix will get included in the shell's prompt variable
    /// verbatim except that the string '$SHPOOL_SESSION_NAME' will
    /// get replaced with the actual name of the shpool session.
    ///
    /// To disable the prompt prefix entirely, simply set a blank
    /// prompt prefix (`prompt_prefix = ""`). You can then optionally
    /// make your own prompt shpool aware by examining the SHPOOL_SESSION_NAME
    /// environment variable.
    pub prompt_prefix: Option<String>,

    /// Control when and how shpool will display the message of the day.
    pub motd: Option<MotdDisplayMode>,

    /// Override arguments to pass to pam_motd.so when resolving the
    /// message of the day. Normally, you want to leave this blank
    /// so that shpool will scrape the default arguments used in
    /// `/etc/pam.d/{ssh,login}` which typically produces the expected
    /// result, but in some cases you may need to override the argument
    /// list. You can also use this to make a custom message of the
    /// day that is only displayed when using shpool.
    ///
    /// See https://man7.org/linux/man-pages/man8/pam_motd.8.html
    /// for more info.
    pub motd_args: Option<Vec<String>>,
}

impl Config {
    /// Merge with `another` Config instance, with `self` taking higher
    /// priority, i.e. it is not commutative.
    ///
    /// Top level options with simple value are directly taken from `self`.
    /// List or map fields may be handled differently. If so, it will be
    /// highlighted in that field's documentation.
    pub fn merge(self, another: Config) -> Config {
        Config {
            norc: self.norc.or(another.norc),
            noecho: self.noecho.or(another.noecho),
            nosymlink_ssh_auth_sock: self
                .nosymlink_ssh_auth_sock
                .or(another.nosymlink_ssh_auth_sock),
            noread_etc_environment: self.noread_etc_environment.or(another.noread_etc_environment),
            nodaemonize: self.nodaemonize.or(another.nodaemonize),
            nodaemonize_timeout: self.nodaemonize_timeout.or(another.nodaemonize_timeout),
            shell: self.shell.or(another.shell),
            env: self.env.or(another.env),
            forward_env: self.forward_env.or(another.forward_env),
            initial_path: self.initial_path.or(another.initial_path),
            session_restore_mode: self.session_restore_mode.or(another.session_restore_mode),
            output_spool_lines: self.output_spool_lines.or(another.output_spool_lines),
            vt100_output_spool_width: self
                .vt100_output_spool_width
                .or(another.vt100_output_spool_width),
            keybinding: self.keybinding.or(another.keybinding),
            prompt_prefix: self.prompt_prefix.or(another.prompt_prefix),
            motd: self.motd.or(another.motd),
            motd_args: self.motd_args.or(another.motd_args),
        }
    }
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

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum MotdDisplayMode {
    /// Never display the message of the day.
    #[default]
    Never,

    /// Display the message of the day using the given program
    /// as the pager. The pager will be invoked like `pager /tmp/motd.txt`,
    /// and normal connection will only proceed once the pager has
    /// exited.
    ///
    /// Display the message of the day each time a user attaches
    /// (wether to a new session or reattaching to an existing session).
    ///
    /// Typically bin is set to `"less"` if you want to use this option.
    Pager {
        /// The path of the binary to use as the pager. It will get invoked
        /// like `bin /tmp/motd.txt`, so it needs to show its first arg
        /// as the content.
        bin: String,
        /// If provided, this option should contain a duration in the
        /// same format accepted by the --ttl flag, and indicates that
        /// shpool should not use the pager to show the motd if it has
        /// already done so within the given duration.
        ///
        /// If this is not provided, shpool will show the motd in a pager
        /// every time the user attaches with no debounce.
        show_every: Option<String>,
    },

    /// Just dump the message of the day directly to the screen.
    /// Dumps are only performed when a new session is created.
    /// There is no safe way to dump directly when reattaching,
    /// so we don't attempt it.
    Dump,
}

#[cfg(test)]
mod test {
    use super::*;
    use ntest::timeout;

    #[test]
    #[timeout(30000)]
    fn parse() -> Result<()> {
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

    mod merge {
        use super::*;
        use assert_matches::assert_matches;

        #[test]
        #[timeout(30000)]
        fn simple_value() -> Result<()> {
            // 4 values are chosen to cover all combinations of None and Some cases.
            let higher = Config {
                norc: None,
                noecho: None,
                shell: Some("abc".to_string()),
                session_restore_mode: Some(SessionRestoreMode::Simple),
                ..Default::default()
            };
            let lower = Config {
                norc: Some(true),
                noecho: None,
                shell: None,
                session_restore_mode: Some(SessionRestoreMode::Lines(42)),
                ..Default::default()
            };

            assert_matches!(higher.merge(lower), Config {
                norc: Some(true),
                noecho: None,
                shell: Some(shell),
                session_restore_mode: Some(SessionRestoreMode::Simple),
                ..
            } if shell == "abc");
            Ok(())
        }

        #[test]
        #[timeout(30000)]
        fn vec_value() -> Result<()> {
            let higher = Config {
                forward_env: Some(vec!["abc".to_string(), "efg".to_string()]),
                motd_args: None,
                ..Default::default()
            };
            let lower = Config {
                forward_env: None,
                motd_args: Some(vec!["hij".to_string(), "klm".to_string()]),
                ..Default::default()
            };

            let actual = higher.merge(lower);
            assert_eq!(actual.forward_env, Some(vec!["abc".to_string(), "efg".to_string()]));
            assert_eq!(actual.motd_args, Some(vec!["hij".to_string(), "klm".to_string()]));
            Ok(())
        }

        #[test]
        #[timeout(30000)]
        fn map_value() -> Result<()> {
            let higher = Config {
                env: Some(HashMap::from([
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                ])),
                ..Default::default()
            };
            let lower = Config {
                env: Some(HashMap::from([
                    ("key3".to_string(), "value3".to_string()),
                    ("key4".to_string(), "value4".to_string()),
                ])),
                ..Default::default()
            };

            let actual = higher.merge(lower);

            assert_eq!(
                actual.env,
                Some(HashMap::from([
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                ]))
            );
            Ok(())
        }
    }
}
