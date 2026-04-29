use std::process::Command;

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::DaemonArgs;

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let out = daemon_proc.var_list(false)?;
    assert!(out.status.success(), "var list proc did not exit successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert_eq!(stderr.len(), 0, "expected no stderr");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.trim().len(), 0, "expected no stdout");

    Ok(())
}

#[test]
#[timeout(30000)]
fn set_get() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let out = daemon_proc.var_set("foo", "bar")?;
    assert!(out.status.success(), "var set proc did not exit successfully");

    let out = daemon_proc.var_get("foo")?;
    assert!(out.status.success(), "var get proc did not exit successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.trim(), "bar");

    Ok(())
}

#[test]
#[timeout(30000)]
fn list() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    daemon_proc.var_set("k1", "v1")?;
    daemon_proc.var_set("k2", "v2")?;

    let out = daemon_proc.var_list(false)?;
    assert!(out.status.success(), "var list proc did not exit successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert!(stdout.contains("k1\tv1"));
    assert!(stdout.contains("k2\tv2"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn unset() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    daemon_proc.var_set("foo", "bar")?;
    let out = daemon_proc.var_get("foo")?;
    assert_eq!(String::from_utf8_lossy(&out.stdout[..]).trim(), "bar");

    let out = daemon_proc.var_unset("foo")?;
    assert!(out.status.success(), "var unset proc did not exit successfully");

    let out = daemon_proc.var_get("foo")?;
    assert!(out.status.success(), "var get proc did not exit successfully");
    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    assert_eq!(stdout.trim().len(), 0);

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_output() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    daemon_proc.var_set("k1", "v1")?;

    let out = daemon_proc.var_list(true)?;
    assert!(out.status.success(), "var list --json proc did not exit successfully");

    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).context("parsing JSON output")?;

    assert_eq!(parsed["k1"], "v1");

    Ok(())
}

#[test]
#[timeout(30000)]
fn no_daemon() -> anyhow::Result<()> {
    let out = Command::new(support::shpool_bin()?)
        .arg("--socket")
        .arg("/fake/does/not/exist/shpool.socket")
        .arg("--no-daemonize")
        .arg("var")
        .arg("list")
        .output()
        .context("spawning var list proc")?;

    assert!(!out.status.success(), "var list proc exited successfully");

    let stderr = String::from_utf8_lossy(&out.stderr[..]);
    assert!(stderr.contains("could not connect to daemon"));

    Ok(())
}
