use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    process::Command,
    sync::mpsc,
    time::Duration,
};

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::DaemonArgs;

/// Regression test for a deadlock where the global `shells` mutex is held
/// during `spawn_subshell` -> `wait_for_startup`. If `wait_for_startup`
/// blocks (e.g. the shell never produces the startup sentinel), the mutex
/// is held forever, blocking ALL daemon operations (list, attach, detach,
/// kill) that need it.
///
/// This test:
/// 1. Creates a "shell" that just sleeps (never produces the sentinel)
/// 2. Uses a config with non-empty prompt_prefix (triggers sentinel injection)
/// 3. Spawns an attach (which hangs in wait_for_startup, holding the mutex)
/// 4. Tries `list` — on buggy code this deadlocks; on fixed code it returns
#[test]
#[timeout(20000)]
fn list_not_blocked_by_slow_shell_spawn() -> anyhow::Result<()> {
    // Create a "shell" that never produces the startup sentinel.
    let tmp_dir = tempfile::tempdir().context("creating temp dir")?;

    let hanging_shell = tmp_dir.path().join("hanging_shell.sh");
    {
        let mut f = std::fs::File::create(&hanging_shell)?;
        writeln!(f, "#!/bin/bash")?;
        writeln!(f, "# A shell that never starts up properly.")?;
        writeln!(f, "# It reads stdin to avoid SIGPIPE but never executes commands.")?;
        writeln!(f, "exec cat > /dev/null")?;
    }
    std::fs::set_permissions(&hanging_shell, std::fs::Permissions::from_mode(0o755))?;

    // Create config that uses the hanging shell with a non-empty prompt_prefix.
    // The non-empty prompt_prefix is key: it causes supports_sentinels=true,
    // which triggers the call to wait_for_startup inside spawn_subshell.
    let config_path = tmp_dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
norc = true
noecho = true
shell = "{}"
session_restore_mode = "simple"
prompt_prefix = "test> "

[env]
PS1 = "prompt> "
TERM = ""
"#,
            hanging_shell.display()
        ),
    )?;

    // Start the daemon with this config (no test_hook events needed).
    let mut daemon_proc = support::daemon::Proc::new(
        &config_path,
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    // Spawn an attach in the background.
    // This will enter select_shell_desc, lock(shells), call spawn_subshell,
    // and hang in wait_for_startup — WITH THE MUTEX HELD on buggy code.
    let socket_for_attach = daemon_proc.socket_path.clone();
    let bin = support::shpool_bin()?;
    let mut attach_child = Command::new(&bin)
        .arg("-vv")
        .arg("--socket")
        .arg(&socket_for_attach)
        .arg("--config-file")
        .arg(&config_path)
        .arg("--no-daemonize")
        .arg("attach")
        .arg("deadlock-test-session")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning attach proc")?;

    // Give the attach time to reach wait_for_startup and grab the mutex.
    std::thread::sleep(Duration::from_secs(3));

    // Now try `list` in a background thread with a timeout.
    // On BUGGY code: list blocks forever (deadlock on shells mutex).
    // On FIXED code: list returns immediately.
    let (tx, rx) = mpsc::channel();
    let socket_for_list = daemon_proc.socket_path.clone();
    let bin_for_list = support::shpool_bin()?;
    std::thread::spawn(move || {
        let result = Command::new(&bin_for_list)
            .arg("-vv")
            .arg("--socket")
            .arg(&socket_for_list)
            .arg("--no-daemonize")
            .arg("list")
            .output();
        let _ = tx.send(result);
    });

    // Wait for list to complete, with a 5-second timeout.
    let list_result = rx.recv_timeout(Duration::from_secs(5));

    // Clean up: kill the hanging attach process.
    let _ = attach_child.kill();
    let _ = attach_child.wait();

    match list_result {
        Ok(Ok(output)) => {
            assert!(
                output.status.success(),
                "list should succeed, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("NAME"), "list output should contain headers");
            Ok(())
        }
        Ok(Err(e)) => {
            panic!("list command failed to execute: {:?}", e);
        }
        Err(_) => {
            panic!(
                "DEADLOCK DETECTED: `shpool list` did not complete within 5 seconds. \
                 The shells mutex is being held by spawn_subshell/wait_for_startup, \
                 blocking all other daemon operations."
            );
        }
    }
}
