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

    let config_tmpl = fs::read_to_string(support::testdata_file("custom_shell.toml.tmpl"))?;
    let config_contents = config_tmpl
        .replace("SHELL", support::testdata_file("hang_shell.sh").to_string_lossy().as_ref());
    let config_file = tmp_dir.path().join("custom_shell.toml");
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

/// Regression test for a bug where shpool would loop forever if the shell
/// exited immediately during startup while we were waiting for the
/// startup sentinel.
#[test]
#[timeout(10000)]
fn no_loop_on_shell_exit_during_startup() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;

    let config_tmpl = fs::read_to_string(support::testdata_file("custom_shell.toml.tmpl"))?;
    // Use /bin/true as the shell so it exits immediately.
    // We need to trigger wait_for_startup, which happens when prompt_prefix is set.
    let config_contents = config_tmpl.replace("SHELL", "/bin/true");
    let config_file = tmp_dir.path().join("exit_shell.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    // Try to attach. This should trigger wait_for_startup because prompt_prefix is
    // set in hang_shell.toml.tmpl.
    // The shell (/bin/true) will exit immediately, causing wait_for_startup to get
    // EOF. On BUGGY code: this loops forever in the daemon.
    // On FIXED code: this returns an error in the daemon, and the attach proc
    // finishes.
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // Wait for the attach process to exit.
    let _status = attach_proc.proc.wait().context("waiting for attach proc")?;

    Ok(())
}

/// Regression test for a bug where shpool would fail to spawn new shells
/// if the binary was overwritten (as happens during package updates).
/// When a binary is overwritten, `std::env::current_exe()` returns a path
/// ending in " (deleted)". We need to strip this to correctly self-exec.
#[test]
#[timeout(30000)]
fn replaced_binary_can_still_spawn_shells() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    let bin_dir = tmp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir)?;
    let shpool_bin_orig = support::shpool_bin()?;
    let shpool_bin_path = bin_dir.join("shpool");

    let copy_bin = |src_path: &std::path::Path, dst_path: &std::path::Path| -> anyhow::Result<()> {
        let mut src = fs::File::open(src_path)?;
        let mut dst = fs::File::create(dst_path)?;
        std::io::copy(&mut src, &mut dst)?;
        dst.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = dst.metadata()?.permissions();
            perms.set_mode(0o755);
            dst.set_permissions(perms)?;
        }
        Ok(())
    };

    copy_bin(&shpool_bin_orig, &shpool_bin_path)?;
    std::thread::sleep(Duration::from_millis(200));

    let config_file = support::testdata_file("prompt_prefix_bash.toml");

    let daemon_args =
        DaemonArgs { bin_path: Some(shpool_bin_path.clone()), ..DaemonArgs::default() };
    let mut daemon_proc =
        support::daemon::Proc::new(&config_file, daemon_args).context("starting daemon proc")?;

    // First attach should work fine.
    let _attach_proc1 =
        daemon_proc.attach("sh1", Default::default()).context("starting first attach proc")?;
    daemon_proc.await_event("wait-for-startup-enter")?;
    // Give it a moment to finish startup
    std::thread::sleep(Duration::from_millis(100));

    // Remove the binary and restore it.
    // This simulates a package update where the old binary is unlinked
    // and a new one is put in its place.
    fs::remove_file(&shpool_bin_path)?;
    copy_bin(&shpool_bin_orig, &shpool_bin_path)?;
    std::thread::sleep(Duration::from_millis(200));

    // Second attach should also work.
    // On BUGGY code: this fails because the daemon tries to exec ".../shpool
    // (deleted) daemon" which does not exist.
    let _attach_proc2 =
        daemon_proc.attach("sh2", Default::default()).context("starting second attach proc")?;
    daemon_proc.await_event("wait-for-startup-enter")?;
    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    Ok(())
}
