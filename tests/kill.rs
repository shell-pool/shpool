use std::{env, process::Command};

use anyhow::Context;
use ntest::timeout;

mod support;

#[test]
#[timeout(30000)]
fn no_daemon() -> anyhow::Result<()> {
    support::dump_err(|| {
        let out = Command::new(support::shpool_bin()?)
            .arg("--socket")
            .arg("/fake/does/not/exist/shpool.socket")
            .arg("kill")
            .output()
            .context("spawning kill proc")?;

        assert!(!out.status.success(), "kill proc exited successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("could not connect to daemon"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;

        env::remove_var("SHPOOL_SESSION_NAME");

        let out = daemon_proc.kill(vec![])?;
        assert!(!out.status.success());

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        eprintln!("stderr: {}", stderr);
        assert!(stderr.contains("no session to kill"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn single_attached() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);
        let _attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

        let out = daemon_proc.kill(vec![String::from("sh1")])?;
        assert!(out.status.success());

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert!(stdout.len() == 0);

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.len() == 0);

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn multiple_attached() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let mut waiter = daemon_proc
            .events
            .take()
            .unwrap()
            .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-enter"]);
        let _sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let _sess2 =
            daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
        waiter.wait_event("daemon-bidi-stream-enter")?;
        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

        let out = daemon_proc.kill(vec![String::from("sh1"), String::from("sh2")])?;
        assert!(out.status.success());

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert!(stdout.len() == 0);

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.len() == 0);

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn reattach_after_kill() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let waiter =
            daemon_proc.events.take().unwrap().waiter(["daemon-handle-kill-removed-shells"]);

        let mut sess1 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
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

        daemon_proc.events = Some(waiter.wait_final_event("daemon-handle-kill-removed-shells")?);

        let mut sess2 =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut lm2 = sess2.line_matcher()?;
        sess2.run_cmd("echo ${MYVAR:-second}")?;
        lm2.match_re("second$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn single_detached() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

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
        assert!(stdout.len() == 0);

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.len() == 0);

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn multiple_detached() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

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
        assert!(stdout.len() == 0);

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.len() == 0);

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn multiple_mixed() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

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

        let _sess2 =
            daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
        daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

        let out = daemon_proc.kill(vec![String::from("sh1"), String::from("sh2")])?;
        assert!(out.status.success());

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert!(stdout.len() == 0);

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.len() == 0);

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn running_env_var() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

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
    })
}

#[test]
#[timeout(30000)]
fn missing() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let out = daemon_proc.kill(vec![String::from("missing")])?;
        assert!(!out.status.success());

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("not found: missing"));

        Ok(())
    })
}
