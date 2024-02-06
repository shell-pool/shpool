use std::process::Command;

use anyhow::Context;
use ntest::timeout;
use regex::Regex;

mod support;

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;
        let out = daemon_proc.list()?;
        assert!(out.status.success(), "list proc did not exit successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert_eq!(stderr.len(), 0, "expected no stderr");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert!(stdout.contains("NAME"));
        assert!(stdout.contains("STARTED_AT"));

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
            .arg("list")
            .output()
            .context("spawning list proc")?;

        assert!(!out.status.success(), "list proc exited successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("could not connect to daemon"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn one_session() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);

        let _sess1 = daemon_proc.attach("sh1", Default::default())?;

        daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

        let out = daemon_proc.list()?;
        assert!(out.status.success(), "list proc did not exit successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert_eq!(stderr.len(), 0, "expected no stderr");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        assert!(stdout.contains("sh1"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn two_sessions() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut bidi_enter_w = daemon_proc.events.take().unwrap().waiter([
            "daemon-bidi-stream-enter",
            "daemon-bidi-stream-enter",
            "daemon-bidi-stream-done",
        ]);

        let _sess1 = daemon_proc.attach("sh1", Default::default())?;

        bidi_enter_w.wait_event("daemon-bidi-stream-enter")?;

        {
            let _sess2 = daemon_proc.attach("sh2", Default::default())?;

            bidi_enter_w.wait_event("daemon-bidi-stream-enter")?;

            let out = daemon_proc.list()?;
            assert!(out.status.success(), "list proc did not exit successfully");

            let stderr = String::from_utf8_lossy(&out.stderr[..]);
            assert_eq!(stderr.len(), 0, "expected no stderr");

            let stdout = String::from_utf8_lossy(&out.stdout[..]);
            assert!(stdout.contains("sh1"));
            assert!(stdout.contains("sh2"));
        }

        // wait for the hangup to complete
        bidi_enter_w.wait_event("daemon-bidi-stream-done")?;

        let out = daemon_proc.list()?;
        assert!(out.status.success(), "list proc did not exit successfully");

        let sh1_re = Regex::new("sh1.*attached")?;
        let sh2_re = Regex::new("sh2.*disconnected")?;
        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        dbg!(&stdout);
        assert!(sh1_re.is_match(&stdout));
        assert!(sh2_re.is_match(&stdout));

        Ok(())
    })
}
