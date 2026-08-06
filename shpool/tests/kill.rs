#![allow(clippy::literal_string_with_formatting_args)]

use std::{env, process::Command};

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::DaemonArgs;

#[test]
#[timeout(30000)]
fn no_daemon() -> anyhow::Result<()> {
    let out = Command::new(support::shpool_bin()?)
        .arg("--socket")
        .arg("/fake/does/not/exist/shpool.socket")
        .arg("--no-daemonize")
        .arg("kill")
        .output()
        .context("spawning kill proc")?;

    assert!(!out.status.success(), "kill proc exited successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.contains("could not connect to daemon"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    // Safety: I think this is actually wrong because tests can run
    // in parallel. It hasn't ever caused a problem in practice though,
    // and this is just a test, so I think it's fine.
    unsafe {
        env::remove_var("SHPOOL_SESSION_NAME");
    }

    let out = daemon_proc.kill(vec![])?;
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    eprintln!("stderr: {stderr}");
    assert!(stderr.contains("no session to kill"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_newer() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs {
            extra_env: vec![(String::from("SHPOOL_TEST__OVERRIDE_VERSION"), String::from("0.0.0"))],
            ..DaemonArgs::default()
        },
    )
    .context("starting daemon proc")?;

    let waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // get past the version mismatch prompt
    attach_proc.run_cmd("")?;

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    println!("stderr: {stderr}");
    assert!(stderr.contains("is newer"));
    assert!(stderr.contains("try restarting"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn single_attached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);
    let _attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_attached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-enter"]);
    let _sess1 = daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let _sess2 = daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    Ok(())
}

#[test]
#[timeout(30000)]
fn reattach_after_kill() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-handle-kill-removed-shells", "daemon-bidi-stream-done"]);

    let mut sess1 =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut lm1 = sess1.line_matcher()?;
    sess1.run_cmd("export MYVAR=first")?;
    sess1.run_cmd("echo $MYVAR")?;
    lm1.scan_until_re("first$")?;

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    waiter.wait_event("daemon-handle-kill-removed-shells")?;
    waiter.wait_event("daemon-bidi-stream-done")?;

    let mut sess2 =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut lm2 = sess2.line_matcher()?;
    sess2.run_cmd("echo ${MYVAR:-second}")?;
    lm2.scan_until_re(".*second.*")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn single_detached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    {
        let _attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_detached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap().waiter([
        "daemon-bidi-stream-enter",
        "daemon-bidi-stream-enter",
        "daemon-bidi-stream-done",
        "daemon-bidi-stream-done",
    ]);

    {
        let _sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let _sess2 =
            daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }
    waiter.wait_event("daemon-bidi-stream-done")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    let out = daemon_proc.kill(vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_mixed() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap().waiter([
        "daemon-bidi-stream-enter",
        "daemon-bidi-stream-done",
        "daemon-bidi-stream-enter",
    ]);

    {
        let _sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }

    waiter.wait_event("daemon-bidi-stream-done")?;

    let _sess2 = daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.is_empty());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.is_empty());

    Ok(())
}

#[test]
#[timeout(30000)]
fn running_env_var() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    let _attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = Command::new(support::shpool_bin()?)
        .arg("--socket")
        .arg(&daemon_proc.socket_path)
        .arg("kill")
        .env("SHPOOL_SESSION_NAME", "sh1")
        .output()
        .context("spawning detach cmd")?;
    assert!(out.status.success(), "not successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.len(), 0, "expected no stdout");

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    Ok(())
}

/// A session whose shell has already died while nothing was attached still
/// sits in the daemon's table. Killing it must succeed and remove it, rather
/// than failing on ESRCH and leaving a session that can never be killed.
#[test]
#[timeout(30000)]
fn already_dead_shell() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    let child_pid: i32;
    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo shellpid=$$")?;
        let caps = line_matcher.scan_until_re_captures("shellpid=([0-9]+)$")?;
        child_pid = caps[1].as_ref().ok_or(anyhow::anyhow!("no pid captured"))?.parse()?;
    }

    // Let the daemon notice the client is gone, so the session is sitting in
    // the table with no one attached.
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    // Kill the shell out from under the daemon, the way an OOM kill or a stray
    // `kill -9` would.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child_pid),
        Some(nix::sys::signal::Signal::SIGKILL),
    )
    .context("killing shell out from under the daemon")?;

    // Wait for the daemon's child watcher to reap it. Until it does the shell
    // is a zombie, and signals to a zombie still succeed.
    let start = std::time::Instant::now();
    while nix::sys::signal::kill(nix::unistd::Pid::from_raw(child_pid), None).is_ok() {
        if start.elapsed() > std::time::Duration::from_secs(10) {
            anyhow::bail!("shell {child_pid} was never reaped");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // On buggy code the SIGHUP fails with ESRCH, the error propagates, and the
    // session is never removed from the table.
    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(out.status.success(), "kill failed: {stderr}");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");

    // and the session should really be gone, not just reported as killed.
    let list_out = daemon_proc.list()?;
    let listing = String::from_utf8_lossy(&list_out.stdout[..]);
    assert!(!listing.contains("sh1"), "session survived the kill: {listing}");

    Ok(())
}

#[test]
#[timeout(30000)]
fn missing() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let out = daemon_proc.kill(vec![String::from("missing")])?;
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.contains("not found: missing"));

    Ok(())
}
