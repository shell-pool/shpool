//! Spawn `shpool attach [-f] <name>` as a child process.
//!
//! We shell out (to ourselves, via `argv[0]`) rather than call
//! [`crate::attach::run`] in-process because attach owns the terminal
//! in raw mode and spawns its own stdin/stdout threads — see
//! libshpool/src/protocol.rs pipe_bytes(). A subprocess boundary is
//! the simplest way to make sure that all cleans up before the TUI
//! reclaims the screen.

use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Context, Result};

/// Parent-invocation flags to forward to the spawned `shpool attach`.
///
/// When the user runs `shpool --config-file foo.toml tui`, the spawned
/// attaches need to see the same `--config-file foo.toml` so the
/// attached shell picks up the right config (prompt template, hooks,
/// TTL defaults, etc.). Same reasoning for `--log-file` and `-v`.
/// `--socket` lives here too so every child talks to the same daemon.
///
/// What's deliberately *not* forwarded: `--daemonize`. The spawned
/// attach is always given `--no-daemonize`, even if the parent
/// `shpool tui` was launched with `--daemonize` on. Rationale:
/// silently auto-spawning a daemon from inside an alt-screen attach
/// is worse UX than a visible "daemon's gone, please restart" error,
/// and `shpool tui`'s framing is "manage existing sessions" —
/// starting new infrastructure mid-session doesn't fit. The CLI-flag
/// asymmetry here is small and deliberate.
#[derive(Debug)]
pub struct AttachEnv {
    pub socket: PathBuf,
    pub config_file: Option<String>,
    pub log_file: Option<String>,
    pub verbose: u8,
}

/// Spawn `shpool attach [-f] <name>` and block until it exits.
///
/// The caller (suspend.rs) is responsible for putting the terminal
/// back in cooked mode and leaving the alt screen first. Here we
/// just fork+exec+wait.
///
/// Returns `true` if the child exited cleanly (status 0), `false`
/// otherwise. Errors are reserved for "we couldn't even spawn it".
pub fn spawn_attach(name: &str, force: bool, env: &AttachEnv) -> Result<bool> {
    // Use argv[0] of the current process rather than a hardcoded
    // "shpool". This matters for:
    //   1. Running from a non-installed build (cargo run) where `shpool` on PATH
    //      might be an older system install.
    //   2. Tests that invoke the binary with an explicit path.
    //   3. Any packaging that names the binary differently.
    //
    // libshpool already does this in daemonize::maybe_fork_daemon,
    // so we're consistent with the rest of the crate.
    let arg0 = env::args()
        .next()
        .ok_or_else(|| anyhow!("argv[0] missing — cannot find shpool binary to re-exec"))?;

    let mut cmd = build_command(&arg0, name, force, env);

    // Inherit stdin/stdout/stderr so the child owns the terminal.
    // (This is the default for Command, but being explicit prevents
    // future confusion if someone adds `.stdout(Stdio::piped())`
    // thinking it's harmless.)
    cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let status = cmd.status().context("spawning `shpool attach`")?;
    Ok(status.success())
}

/// Build the `shpool attach` Command, including all forwarded flags.
///
/// Split out from `spawn_attach` so we can unit-test the arg list
/// without actually fork+exec'ing. stdio inheritance is applied by
/// `spawn_attach`; this function does not touch it.
fn build_command(arg0: &str, name: &str, force: bool, env: &AttachEnv) -> Command {
    let mut cmd = Command::new(arg0);

    // Forward top-level invocation flags so the child behaves the
    // same as the parent `shpool tui` was launched with:
    //   --config-file: the child re-reads config for shell setup
    //                  (prompt prefix, hooks, TTL defaults). Without
    //                  forwarding, a user who ran
    //                  `shpool --config-file custom.toml tui` sees
    //                  attached sessions using the *default* config.
    //   --log-file:    match the parent's log destination, else the
    //                  child's diagnostics go nowhere useful.
    //   -v (xN):       match the parent's verbosity.
    //   --socket:      always required — the child has to talk to the
    //                  same daemon we're managing.
    //   --no-daemonize: always forced; see the AttachEnv docstring.
    if let Some(path) = &env.config_file {
        cmd.arg("--config-file").arg(path);
    }
    if let Some(path) = &env.log_file {
        cmd.arg("--log-file").arg(path);
    }
    for _ in 0..env.verbose {
        cmd.arg("-v");
    }
    cmd.arg("--socket").arg(&env.socket).arg("--no-daemonize").arg("attach");
    if force {
        cmd.arg("-f");
    }
    cmd.arg(name);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    fn base_env() -> AttachEnv {
        AttachEnv {
            socket: PathBuf::from("/tmp/shpool.sock"),
            config_file: None,
            log_file: None,
            verbose: 0,
        }
    }

    #[test]
    fn build_command_minimal() {
        let env = base_env();
        let args = args_of(&build_command("shpool", "main", false, &env));
        assert_eq!(args, vec!["--socket", "/tmp/shpool.sock", "--no-daemonize", "attach", "main"],);
    }

    #[test]
    fn build_command_with_force() {
        let env = base_env();
        let args = args_of(&build_command("shpool", "main", true, &env));
        // -f comes after the `attach` subcommand; name is last.
        assert!(
            args.windows(2).any(|w| w[0] == "attach" && w[1] == "-f"),
            "expected `attach -f` pair; got {args:?}",
        );
        assert_eq!(args.last().map(String::as_str), Some("main"));
    }

    #[test]
    fn build_command_forwards_config_file() {
        let mut env = base_env();
        env.config_file = Some("/etc/shpool/custom.toml".into());
        let args = args_of(&build_command("shpool", "main", false, &env));
        assert!(
            args.windows(2).any(|w| w[0] == "--config-file" && w[1] == "/etc/shpool/custom.toml"),
            "expected --config-file with path; got {args:?}",
        );
    }

    #[test]
    fn build_command_forwards_log_file() {
        let mut env = base_env();
        env.log_file = Some("/var/log/shpool.log".into());
        let args = args_of(&build_command("shpool", "main", false, &env));
        assert!(
            args.windows(2).any(|w| w[0] == "--log-file" && w[1] == "/var/log/shpool.log"),
            "expected --log-file with path; got {args:?}",
        );
    }

    #[test]
    fn build_command_forwards_verbose_count() {
        let mut env = base_env();
        env.verbose = 3;
        let args = args_of(&build_command("shpool", "main", false, &env));
        assert_eq!(args.iter().filter(|a| *a == "-v").count(), 3);
    }

    #[test]
    fn build_command_no_verbose_when_zero() {
        let env = base_env();
        let args = args_of(&build_command("shpool", "main", false, &env));
        assert!(!args.iter().any(|a| a == "-v"));
    }

    #[test]
    fn build_command_global_flags_precede_subcommand() {
        // Clap requires global flags (those declared on `Args`) to
        // appear before the subcommand (`attach`). Check the ordering
        // holds when every forwardable flag is set.
        let env = AttachEnv {
            socket: PathBuf::from("/tmp/s"),
            config_file: Some("/c".into()),
            log_file: Some("/l".into()),
            verbose: 1,
        };
        let args = args_of(&build_command("shpool", "main", false, &env));
        let attach_pos = args.iter().position(|a| a == "attach").expect("attach present");
        for flag in ["--config-file", "--log-file", "-v", "--socket", "--no-daemonize"] {
            let pos = args.iter().position(|a| a == flag).expect(flag);
            assert!(pos < attach_pos, "{flag} should come before `attach`; got {args:?}");
        }
    }
}
