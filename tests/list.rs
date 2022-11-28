use std::process::Command;

use anyhow::Context;

mod support;

#[test]
fn empty() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let out = daemon_proc.list()?;
    assert!(out.status.success(),
            "list proc did not exit successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("NAME"));
    assert!(stdout.contains("STARTED_AT"));

    Ok(())
}

#[test]
fn no_daemon() -> anyhow::Result<()> {
    let out = Command::new(support::shpool_bin())
        .arg("--socket").arg("/fake/does/not/exist/shpool.socket")
        .arg("list")
        .output()
        .context("spawning list proc")?;

    assert!(!out.status.success(),
            "list proc exited successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("could not connect to daemon"));

    Ok(())
}

#[test]
fn one_session() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let bidi_enter_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter"]);

    let _sess1 = daemon_proc.attach("sh1")?;

    daemon_proc.events = Some(bidi_enter_w.wait_final_event(
            "daemon-bidi-stream-enter")?);

    let out = daemon_proc.list()?;
    assert!(out.status.success(),
            "list proc did not exit successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("sh1"));

    Ok(())
}

#[test]
fn two_sessions() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut bidi_enter_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-enter",
                "daemon-bidi-stream-enter"]);

    let _sess1 = daemon_proc.attach("sh1")?;

    bidi_enter_w.wait_event("daemon-bidi-stream-enter")?;

    let _sess2 = daemon_proc.attach("sh2")?;

    daemon_proc.events = Some(bidi_enter_w.wait_final_event(
            "daemon-bidi-stream-enter")?);

    let out = daemon_proc.list()?;
    assert!(out.status.success(),
            "list proc did not exit successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("sh1"));
    assert!(stdout.contains("sh2"));

    Ok(())
}
