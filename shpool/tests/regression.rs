use std::{fs, io::Write, process::Command, sync::mpsc, time::Duration};

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::{daemon::DaemonArgs, tmpdir};

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
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;

    let config_tmpl = fs::read_to_string(support::testdata_file("hang_shell.toml.tmpl"))?;
    let config_contents = config_tmpl
        .replace("SHELL", support::testdata_file("hang_shell.sh").to_string_lossy().as_ref());
    let config_file = tmp_dir.path().join("motd_dump.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    let _attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    // not really needed, just here to test the events system
    daemon_proc.await_event("wait-for-startup-enter")?;

    // Now try `list` in a background thread with a timeout.
    // On BUGGY code: list blocks forever (deadlock on shells mutex).
    // On FIXED code: list returns immediately.
    let (tx, rx) = mpsc::channel();
    let socket_for_list = daemon_proc.socket_path.clone();
    let shpool_bin = support::shpool_bin()?;
    std::thread::spawn(move || {
        let result = Command::new(&shpool_bin)
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
