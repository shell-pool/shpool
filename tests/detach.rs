use std::process::Command;

use anyhow::Context;

mod support;

#[test]
fn single_running() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    let _attach_proc = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(out.status.success(), "not successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.len(), 0, "expected no stdout");

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    Ok(())
}

#[test]
fn single_not_running() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let out = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(!out.status.success(), "successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("not found: sh1"), "expected no stdout");

    Ok(())
}

#[test]
fn no_daemon() -> anyhow::Result<()> {
    let out = Command::new(support::shpool_bin()?)
        .arg("--socket")
        .arg("/fake/does/not/exist/shpool.socket")
        .arg("detach")
        .output()
        .context("spawning detach proc")?;

    assert!(!out.status.success(), "detach proc exited successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("could not connect to daemon"));

    Ok(())
}

#[test]
fn running_env_var() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    let _attach_proc = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = Command::new(support::shpool_bin()?)
        .arg("--socket")
        .arg(&daemon_proc.socket_path)
        .arg("detach")
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

#[test]
fn reattach() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let bidi_done_w = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-done"]);
    let mut sess1 = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;

    let mut lm1 = sess1.line_matcher()?;
    sess1.run_cmd("export MYVAR=first ; echo hi")?;
    lm1.match_re("hi$")?;

    let out = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(out.status.success(), "not successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.len(), 0, "expected no stdout");

    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    let mut sess2 = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    let mut lm2 = sess2.line_matcher()?;
    sess2.run_cmd("echo ${MYVAR:-second}")?;
    lm2.match_re("first$")?;

    Ok(())
}

#[test]
fn multiple_running() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap().waiter([
        "daemon-bidi-stream-enter",
        "daemon-bidi-stream-enter",
        "daemon-bidi-stream-done",
        "daemon-bidi-stream-done",
    ]);
    let _sess1 = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let _sess2 = daemon_proc
        .attach("sh2", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = daemon_proc.detach(vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success(), "not successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.len(), 0, "expected no stdout");

    waiter.wait_event("daemon-bidi-stream-done")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    Ok(())
}

#[test]
fn multiple_mixed() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    let _attach_proc = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = daemon_proc.detach(vec![String::from("sh1"), String::from("sh2")])?;
    assert!(!out.status.success(), "unexpectedly successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("not found: sh2"), "expected not found");

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    Ok(())
}

#[test]
fn double_tap() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
    let _attach_proc = daemon_proc
        .attach("sh1", false, vec![])
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out1 = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(out1.status.success(), "not successful");

    let stderr1 = String::from_utf8_lossy(&out1.stderr[..]);
    assert_eq!(stderr1.len(), 0, "expected no stderr");

    let stdout1 = String::from_utf8_lossy(&out1.stdout[..]);
    assert_eq!(stdout1.len(), 0, "expected no stdout");

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    let out2 = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(!out2.status.success(), "unexpectedly successful");

    let stderr2 = String::from_utf8_lossy(&out2.stderr[..]);
    assert_eq!(stderr2.len(), 0, "expected no stderr");

    let stdout2 = String::from_utf8_lossy(&out2.stdout[..]);
    assert!(
        stdout2.contains("not attached: sh1"),
        "expected not attached"
    );

    Ok(())
}
