use std::process::Command;

use anyhow::{anyhow, Context};
use ntest::timeout;
use regex::Regex;

mod support;

use crate::support::daemon::DaemonArgs;

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs { listen_events: false, ..DaemonArgs::default() },
        )
        .context("starting daemon proc")?;
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
fn version_mismatch_client_newer() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs {
                listen_events: false,
                extra_env: vec![(
                    String::from("SHPOOL_TEST__OVERRIDE_VERSION"),
                    String::from("0.0.0"),
                )],
                ..Default::default()
            },
        )
        .context("starting daemon proc")?;
        let out = daemon_proc.list()?;
        assert!(out.status.success(), "list proc did not exit successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("is newer"));
        assert!(stderr.contains("try restarting"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_older() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs {
                listen_events: false,
                extra_env: vec![(
                    String::from("SHPOOL_TEST__OVERRIDE_VERSION"),
                    String::from("99999.0.0"),
                )],
                ..Default::default()
            },
        )
        .context("starting daemon proc")?;
        let out = daemon_proc.list()?;
        assert!(out.status.success(), "list proc did not exit successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert!(stderr.contains("is older"));
        assert!(stderr.contains("try restarting"));

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
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
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
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
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

#[test]
#[timeout(30000)]
fn json_output() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
        let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);

        let _sess1 = daemon_proc.attach("sh1", Default::default())?;

        daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

        let out = daemon_proc.list_json()?;
        assert!(out.status.success(), "list --json proc did not exit successfully");

        let stderr = String::from_utf8_lossy(&out.stderr[..]);
        assert_eq!(stderr.len(), 0, "expected no stderr");

        let stdout = String::from_utf8_lossy(&out.stdout[..]);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).context("parsing JSON output")?;

        let sessions = parsed
            .get("sessions")
            .ok_or_else(|| anyhow!("missing 'sessions' field in JSON output"))?
            .as_array()
            .ok_or_else(|| anyhow!("'sessions' is not an array"))?;

        assert!(!sessions.is_empty(), "expected at least one session");

        let first_session = &sessions[0];
        assert!(
            first_session.get("last_connected_at_unix_ms").is_some(),
            "missing 'last_connected_at_unix_ms' field"
        );

        Ok(())
    })
}
