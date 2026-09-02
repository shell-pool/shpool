use std::{fs, io::Write, process::Command, sync::mpsc, time::Duration};

use anyhow::{anyhow, Context};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
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
    // We need to trigger wait_for_startup, which happens when prompt_prefix is
    // set.
    let config_contents = config_tmpl.replace("SHELL", "/bin/true");
    let config_file = tmp_dir.path().join("exit_shell.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    // Try to attach. This should trigger wait_for_startup because prompt_prefix
    // is set in hang_shell.toml.tmpl.
    // The shell (/bin/true) will exit immediately, causing wait_for_startup to
    // get EOF. On BUGGY code: this loops forever in the daemon.
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

/// Regression test for an EOF spin loop in the `client_to_shell` thread.
/// If the client abruptly disconnects, the daemon should quickly detect EOF and
/// exit the thread, rather than looping forever and starving other threads
/// (which can cause timeouts).
#[test]
#[timeout(10000)]
fn client_eof_does_not_spin() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // wait for attach to finish startup so we know client_to_shell thread is
    // running
    daemon_proc.await_event("daemon-bidi-stream-enter")?;
    std::thread::sleep(Duration::from_millis(100)); // give it a little more
                                                    // time to enter the loop

    // kill the attach proc abruptly. This closes the socket, sending EOF to
    // client_to_shell.
    attach_proc.proc.kill()?;

    // wait for the session to become disconnected
    // On BUGGY code: client_to_shell spins, causing bidi_stream to wait for
    // heartbeat_h which takes >= 500ms (often longer due to CPU starvation).
    // On FIXED code: client_to_shell exits immediately on EOF, bidi_stream
    // detects it within JOIN_POLL_DURATION (50ms).
    daemon_proc.wait_until_list_matches(|out| out.contains("disconnected"))?;

    Ok(())
}

/// Regression test for an EOF spin loop in the pager display thread.
/// If the client abruptly disconnects while viewing the MOTD pager, the daemon
/// should quickly detect EOF and abort the pager display.
#[test]
#[timeout(10000)]
fn pager_eof_does_not_spin() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    let motd_file = tmp_dir.path().join("motd.txt");
    fs::write(&motd_file, "this is a long motd that you must read in a pager\n")?;

    let config_tmpl = fs::read_to_string(support::testdata_file("motd_pager.toml.tmpl"))?;
    let config_contents = config_tmpl.replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap());
    let config_file = tmp_dir.path().join("motd_pager.toml");
    fs::write(&config_file, config_contents)?;

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // wait for the attach process to launch the pager
    std::thread::sleep(Duration::from_millis(500));

    // kill the attach proc abruptly. This closes the socket, sending EOF to the
    // pager thread.
    attach_proc.proc.kill()?;

    // Wait for the session to become disconnected.
    // On BUGGY code: the pager thread spins 100% CPU on EOF, and never returns
    // from display(). The session remains "Attached" forever (or until the
    // 10s test timeout). On FIXED code: the pager detects EOF, exits, and
    // the session quickly becomes "Disconnected".
    daemon_proc.wait_until_list_matches(|out| out.contains("disconnected"))?;

    Ok(())
}

/// Regression test for a race condition where concurrent attach attempts to
/// a disconnected session can clobber each other's stream, leading to
/// "no client stream" error in the daemon.
#[test]
#[timeout(30000)]
fn concurrent_attach_to_existing_session_race() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_first =
        daemon_proc.attach("main", Default::default()).context("starting first attach proc")?;
    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    attach_first.proc.kill()?;
    daemon_proc.wait_until_list_matches(|out| out.contains("disconnected"))?;

    // Pause the daemon before it locks the session to trigger the race.
    daemon_proc.send_event_command("pause-at handle-attach-before-inner-session-lock")?;

    let _attach_a = daemon_proc.attach("main", Default::default()).context("starting attach A")?;
    daemon_proc.await_event("paused-at handle-attach-before-inner-session-lock")?;

    let _attach_b = daemon_proc.attach("main", Default::default()).context("starting attach B")?;
    daemon_proc.await_event("handle-attach-before-select-shell")?;

    // In fixed code, attach b can't finish select shell because of a lock, so
    // we need to use a sleep here to allow it to enter in broken code.
    std::thread::sleep(Duration::from_millis(500));

    daemon_proc.send_event_command("release handle-attach-before-inner-session-lock")?;

    // Disconnect B to trigger the error for A which is using B's stream.
    drop(_attach_b);

    std::thread::sleep(Duration::from_millis(500));

    let log_content = fs::read_to_string(&daemon_proc.log_file)?;

    // On buggy code, the clobbered stream causes a "no client stream" error
    // when the second attach tries to take over after the first exits.
    assert!(
        !log_content.contains("no client stream, should be impossible"),
        "REGRESSION: Daemon logged 'no client stream' error!"
    );

    Ok(())
}

/// Regression test for a slow heartbeat ack killing the session. The
/// shell->client thread can be busy when the heartbeat thread asks it for an
/// ack (generating a large session restore buffer is the usual cause). Treating
/// that as fatal tore down the thread scope along with the session's
/// shell->client thread, leaving a session that could never be attached again.
///
/// We stall the shell->client thread right after it writes a heartbeat but
/// before it acks, hold it past the ack timeout, then check the session still
/// works.
#[test]
#[timeout(30000)]
fn slow_heartbeat_ack_does_not_wedge_session() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    daemon_proc.send_event_command("pause-at daemon-wrote-heartbeat")?;
    daemon_proc.await_event("paused-at daemon-wrote-heartbeat")?;

    // SHELL_TO_CLIENT_CTL_TIMEOUT is 300ms, so this guarantees the ack recv
    // times out at least once.
    std::thread::sleep(Duration::from_millis(600));

    daemon_proc.send_event_command("release daemon-wrote-heartbeat")?;

    // On buggy code the heartbeat thread has already returned an error, the
    // scope has unwound, and this command never produces output.
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo still-alive")?;
    line_matcher.scan_until_re("still-alive$")?;

    Ok(())
}

/// Regression test for a bug where shpool would abort the attach process
/// if the MOTD pager exited normally (e.g. less EOF). It should transition
/// to the shell instead.
#[test]
#[timeout(15000)]
fn pager_exit_transitions_to_shell() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    let motd_file = tmp_dir.path().join("motd.txt");
    fs::write(&motd_file, "this is a motd\n")?;

    let config_tmpl = fs::read_to_string(support::testdata_file("motd_pager.toml.tmpl"))?;
    let config_contents = config_tmpl
        .replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap())
        .replace("bin = \"less\"", "bin = \"cat\"");
    let config_file = tmp_dir.path().join("motd_pager.toml");
    fs::write(&config_file, config_contents)?;

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // We should be able to run a command in the shell.
    attach_proc.run_cmd("echo 'in-shell'")?;
    line_matcher.scan_until_re("in-shell$")?;

    Ok(())
}

/// Regression test for the attach client discarding the shell's real exit
/// status. The client stamps its fallback status of 1 into the result slot as
/// soon as EITHER pipe thread finishes. A terminal that closes the client's
/// stdin before the shell's ExitStatus frame arrives -- which any terminal
/// does when it hangs up first -- therefore reported 1 no matter how the
/// shell actually exited.
///
/// We close the client's stdin while the shell is still sleeping toward a
/// distinctive exit code, so the stdin->sock thread always finishes first.
#[test]
#[timeout(30000)]
fn stdin_close_does_not_discard_exit_status() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo ready")?;
    line_matcher.scan_until_re("ready$")?;

    // MAX_DETACH_WAIT_DUR is 300ms: the frame must arrive inside the bounded
    // window the client waits after its stdin side finishes.
    attach_proc.run_cmd("sleep 0.15; exit 19")?;
    // Close our stdin immediately: the fallback status must not win the race
    // against the ExitStatus frame that arrives moments later.
    drop(attach_proc.proc.stdin.take());

    let code = attach_proc
        .proc
        .wait()
        .context("waiting for attach proc to exit")?
        .code()
        .ok_or(anyhow!("no exit code"))?;
    assert_eq!(code, 19, "shell exit status was discarded");

    Ok(())
}

/// Regression test for a daemon-wide wedge in the detach handler. The
/// client_connection/client_connection_ack exchange is a rendezvous, so it
/// only completes while the shell->client thread is parked in its select
/// loop. A client whose socket has stopped draining (a stalled ssh window, a
/// suspended laptop) leaves that thread blocked in write() instead, and
/// handle_detach used to run the exchange while still holding the global
/// shells lock -- one unresponsive client wedged every list, attach, detach
/// and kill in the daemon.
///
/// We stop the attach client with SIGSTOP, flood the session with output
/// until the kernel socket buffers fill and the shell->client thread is stuck
/// in write(), then detach. The daemon must answer the detach (reporting the
/// session rather than hanging) and a follow-up list must come back.
#[test]
#[timeout(30000)]
fn detach_of_stalled_client_does_not_wedge_daemon() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    daemon_proc.await_event("daemon-bidi-stream-enter")?;

    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo ready")?;
    line_matcher.scan_until_re("ready$")?;

    // Ask the shell for far more output than the socket buffers hold, then
    // immediately stop the client so nothing drains.
    attach_proc.run_cmd("yes | head -c 8000000; echo flood-done")?;
    let client_pid = Pid::from_raw(attach_proc.proc.id() as i32);
    signal::kill(client_pid, Signal::SIGSTOP).context("stopping attach client")?;

    // Give the flood time to fill the kernel buffers behind the stopped
    // client so the shell->client thread is genuinely parked in write().
    std::thread::sleep(Duration::from_millis(1500));

    // On buggy code this call never returns: the rendezvous send blocks
    // under the shells lock and the whole daemon wedges behind it. The exit
    // status does not matter here -- a stalled client is correctly reported
    // as not attached -- only that the daemon answered at all.
    let _detach_out =
        daemon_proc.detach(vec![String::from("sh1")]).context("detaching stalled client")?;

    // The real assertion: the daemon still answers.
    let list_out = daemon_proc.list().context("listing after detach")?;
    assert!(list_out.status.success(), "list did not complete");

    let _ = signal::kill(client_pid, Signal::SIGCONT);

    Ok(())
}
