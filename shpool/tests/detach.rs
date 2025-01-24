#![allow(clippy::literal_string_with_formatting_args)]

use std::process::Command;

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::DaemonArgs;

#[test]
#[timeout(30000)]
fn single_running() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;

        let mut waiter = daemon_proc
            .events
            .take()
            .unwrap()
            .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;

        let out = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(out.status.success(), "not successful");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert_eq!(stderr.len(), 0, "expected no stderr");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert_eq!(stdout.len(), 0, "expected no stdout");

        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

        let attach_exit_status = attach_proc.proc.wait()?;
        assert!(attach_exit_status.success());

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_newer() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs {
                extra_env: vec![(
                    String::from("SHPOOL_TEST__OVERRIDE_VERSION"),
                    String::from("0.0.0"),
                )],
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

        let out = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(out.status.success());

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("is newer"));
        assert!(stderr.contains("try restarting"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn single_not_running() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs { listen_events: false, ..DaemonArgs::default() },
        )
        .context("starting daemon proc")?;

        let out = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(!out.status.success(), "successful");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("not found: sh1"));

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert_eq!(stdout.len(), 0, "expected no stderr");

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn no_daemon() -> anyhow::Result<()> {
    support::dump_err(|| {
        let out = Command::new(support::shpool_bin()?)
            .arg("--socket")
            .arg("/fake/does/not/exist/shpool.socket")
            .arg("--no-daemonize")
            .arg("detach")
            .output()
            .context("spawning detach proc")?;

        assert!(!out.status.success(), "detach proc exited successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("could not connect to daemon"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn running_env_var() -> anyhow::Result<()> {
    support::dump_err(|| {
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
    })
}

#[test]
#[timeout(30000)]
fn reattach() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;

        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
        let mut sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut lm1 = sess1.line_matcher()?;
        sess1.run_cmd("export MYVAR=first ; echo hi")?;
        lm1.scan_until_re("hi$")?;

        let out = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(out.status.success(), "not successful");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert_eq!(stderr.len(), 0, "expected no stderr");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert_eq!(stdout.len(), 0, "expected no stdout");

        daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

        let mut sess2 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut lm2 = sess2.line_matcher()?;
        sess2.run_cmd("echo ${MYVAR:-second}")?;
        lm2.match_re("first$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn multiple_running() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;

        let mut waiter = daemon_proc.events.take().unwrap().waiter([
            "daemon-bidi-stream-enter",
            "daemon-bidi-stream-enter",
            "daemon-bidi-stream-done",
            "daemon-bidi-stream-done",
        ]);
        let _sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;

        let _sess2 =
            daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
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
    })
}

#[test]
fn multiple_mixed() -> anyhow::Result<()> {
    support::dump_err(|| {
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

        let out = daemon_proc.detach(vec![String::from("sh1"), String::from("sh2")])?;
        assert!(!out.status.success(), "unexpectedly successful");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert_eq!(stdout.len(), 0, "expected no stdout");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("not found: sh2"), "expected not found");

        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

        Ok(())
    })
}

#[test]
fn double_tap() -> anyhow::Result<()> {
    support::dump_err(|| {
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

        let out1 = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(out1.status.success(), "not successful");

        let stdout1 = String::from_utf8_lossy(&out1.stdout[..]);
        assert_eq!(stdout1.len(), 0, "expected no stdout");

        let stderr1 = String::from_utf8_lossy(&out1.stderr[..]);
        assert_eq!(stderr1.len(), 0);

        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

        let out2 = daemon_proc.detach(vec![String::from("sh1")])?;
        assert!(!out2.status.success(), "unexpectedly successful");

        let stdout2 = String::from_utf8_lossy(&out2.stdout[..]);
        assert_eq!(stdout2.len(), 0, "expected no stdout");

        let stderr2 = String::from_utf8_lossy(&out2.stderr[..]);
        assert!(stderr2.contains("not attached: sh1"), "expected not attached");

        Ok(())
    })
}
