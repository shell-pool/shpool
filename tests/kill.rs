use std::process::Command;

use anyhow::Context;

mod support;

#[test]
fn no_daemon() -> anyhow::Result<()> {
    let out = Command::new(support::shpool_bin())
        .arg("--socket").arg("/fake/does/not/exist/shpool.socket")
        .arg("kill")
        .output()
        .context("spawning kill proc")?;

    assert!(!out.status.success(),
            "kill proc exited successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("could not connect to daemon"));

    Ok(())
}

#[test]
fn empty() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let out = daemon_proc.kill(vec![])?;
    assert!(!out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("no session to kill"));

    Ok(())
}

#[test]
fn single_attached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter"]);
    let _attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;
    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    Ok(())
}

#[test]
fn multiple_attached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                "daemon-bidi-stream-enter"]);
    let _sess1 = daemon_proc.attach("sh1")
        .context("starting attach proc")?;
    let _sess2 = daemon_proc.attach("sh2")
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;
    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-enter")?);

    let out = daemon_proc.kill(
        vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    Ok(())
}

#[test]
fn reattach_after_kill() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-handle-kill-removed-shells"]);

    let mut sess1 = daemon_proc.attach("sh1")
        .context("starting attach proc")?;
    let mut lm1 = sess1.line_matcher()?;
    sess1.run_cmd("export MYVAR=first")?;
    sess1.run_cmd("echo $MYVAR")?;
    lm1.match_re("first$")?;

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-handle-kill-removed-shells")?);

    let mut sess2 = daemon_proc.attach("sh1")
        .context("starting attach proc")?;
    let mut lm2 = sess2.line_matcher()?;
    sess2.run_cmd("echo ${MYVAR:-second}")?;
    lm2.match_re("second$")?;

    Ok(())
}

#[test]
fn single_detached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                 "daemon-bidi-stream-done"]);
    {
        let _attach_proc = daemon_proc.attach("sh1")
            .context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }
    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-done")?);

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    Ok(())
}

#[test]
fn multiple_detached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                "daemon-bidi-stream-enter",
                "daemon-bidi-stream-done",
                "daemon-bidi-stream-done"]);

    {
        let _sess1 = daemon_proc.attach("sh1")
            .context("starting attach proc")?;
        let _sess2 = daemon_proc.attach("sh2")
            .context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }
    waiter.wait_event("daemon-bidi-stream-done")?;
    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-done")?);

    let out = daemon_proc.kill(
        vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    Ok(())
}

#[test]
fn multiple_mixed() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                "daemon-bidi-stream-enter",
                "daemon-bidi-stream-done"]);

    {
        let _sess1 = daemon_proc.attach("sh1")
            .context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
    }

    let _sess2 = daemon_proc.attach("sh2")
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-done")?);

    let out = daemon_proc.kill(
        vec![String::from("sh1"), String::from("sh2")])?;
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.len() == 0);

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.len() == 0);

    Ok(())
}

#[test]
fn running_env_var() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                 "daemon-bidi-stream-done"]);
    let _attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let out = Command::new(support::shpool_bin())
        .arg("--socket").arg(&daemon_proc.socket_path)
        .arg("kill")
        .env("SHPOOL_SESSION_NAME", "sh1")
        .output()
        .context("spawning detach cmd")?;
    assert!(out.status.success(), "not successful");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.len(), 0, "expected no stdout");

    daemon_proc.events = Some(waiter.wait_final_event(
            "daemon-bidi-stream-done")?);

    Ok(())
}

#[test]
fn missing() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let out = daemon_proc.kill(vec![String::from("missing")])?;
    assert!(!out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("not found: missing"));

    Ok(())
}
