use std::process::Command;

use anyhow::{anyhow, Context};
use ntest::timeout;
use regex::Regex;
use serde_json::Value;

mod support;

use crate::support::daemon::{AttachArgs, DaemonArgs};

/// Run `shpool list --json` and return the parsed `sessions` array.
fn list_sessions(daemon_proc: &mut support::daemon::Proc) -> anyhow::Result<Vec<Value>> {
    let out = daemon_proc.list_json()?;
    assert!(out.status.success(), "list --json proc did not exit successfully");
    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    let parsed: Value = serde_json::from_str(&stdout).context("parsing JSON output")?;
    Ok(parsed["sessions"].as_array().ok_or_else(|| anyhow!("missing 'sessions' array"))?.clone())
}

#[test]
#[timeout(30000)]
fn empty() -> anyhow::Result<()> {
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
    assert!(stdout.contains("STATUS"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_newer() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs {
            listen_events: false,
            extra_env: vec![(String::from("SHPOOL_TEST__OVERRIDE_VERSION"), String::from("0.0.0"))],
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
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_older() -> anyhow::Result<()> {
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
}

#[test]
#[timeout(30000)]
fn no_daemon() -> anyhow::Result<()> {
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
}

#[test]
#[timeout(30000)]
fn one_session() -> anyhow::Result<()> {
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
}

#[test]
#[timeout(30000)]
fn two_sessions() -> anyhow::Result<()> {
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
        let _sess2 = daemon_proc.attach("sh2_longer_name", Default::default())?;

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

    let sh1_re = Regex::new("sh1            .*attached")?;
    let sh2_re = Regex::new("sh2_longer_name.*disconnected")?;
    let stdout = String::from_utf8_lossy(&out.stdout[..]);
    dbg!(&stdout);
    assert!(sh1_re.is_match(&stdout));
    assert!(sh2_re.is_match(&stdout));

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_output() -> anyhow::Result<()> {
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
    let parsed: serde_json::Value = serde_json::from_str(&stdout).context("parsing JSON output")?;

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
}

#[test]
#[timeout(30000)]
fn json_attachment_literal_name() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);

    let sess1 = daemon_proc.attach("htop", Default::default())?;

    daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

    let sessions = list_sessions(&mut daemon_proc)?;
    let session =
        sessions.iter().find(|s| s["name"] == "htop").ok_or_else(|| anyhow!("htop not found"))?;
    assert_eq!(session["status"], "Attached");

    let attachments =
        session["attachments"].as_array().ok_or_else(|| anyhow!("missing attachments array"))?;
    assert_eq!(attachments.len(), 1, "expected exactly one attachment");
    // A var-free name still has a template: the literal source string.
    assert_eq!(attachments[0]["template"], "htop");
    assert_eq!(attachments[0]["pid"], sess1.proc.id());

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_templated_name() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    daemon_proc.var_set("workspace", "myproj")?;

    let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);

    let sess1 = daemon_proc.attach("{workspace}-edit", Default::default())?;

    daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

    let sessions = list_sessions(&mut daemon_proc)?;
    let session = sessions
        .iter()
        .find(|s| s["name"] == "myproj-edit")
        .ok_or_else(|| anyhow!("myproj-edit not found"))?;
    assert_eq!(session["status"], "Attached");

    let attachments =
        session["attachments"].as_array().ok_or_else(|| anyhow!("missing attachments array"))?;
    assert_eq!(attachments.len(), 1, "expected exactly one attachment");
    // The reported template is the unresolved source, not the resolved name.
    assert_eq!(attachments[0]["template"], "{workspace}-edit");
    assert_eq!(attachments[0]["pid"], sess1.proc.id());

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_cleared_on_detach() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);

    let _sess1 = daemon_proc.attach("sh1", Default::default())?;

    daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

    let sessions = list_sessions(&mut daemon_proc)?;
    let sh1 =
        sessions.iter().find(|s| s["name"] == "sh1").ok_or_else(|| anyhow!("sh1 not found"))?;
    assert_eq!(sh1["status"], "Attached");
    assert_eq!(sh1["attachments"].as_array().unwrap().len(), 1);

    let out = daemon_proc.detach(vec![String::from("sh1")])?;
    assert!(out.status.success(), "detach proc did not exit successfully");

    // The status flip (inner-lock release) and attachment clear race the detach RPC
    // return, so poll until both settle.
    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        Ok(sessions.iter().find(|s| s["name"] == "sh1").is_some_and(|sh1| {
            sh1["status"] == "Disconnected"
                && sh1["attachments"].as_array().is_some_and(|a| a.is_empty())
        }))
    })
    .context("session should be Disconnected with no attachments after detach")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_survives_var_switch() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    daemon_proc.var_set("env", "prod")?;

    let mut attach_proc =
        daemon_proc.attach("{env}-session", Default::default()).context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo $SHPOOL_SESSION_NAME")?;
    line_matcher.scan_until_re("prod-session$")?;

    // The switch re-dials the *same* attach process under the new name, so the
    // template and pid are unchanged; only the session it lands on changes.
    let attach_pid = attach_proc.proc.id();

    daemon_proc.var_set("env", "dev")?;

    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        let prod_ok = sessions.iter().find(|s| s["name"] == "prod-session").is_some_and(|s| {
            s["status"] == "Disconnected"
                && s["attachments"].as_array().is_some_and(|a| a.is_empty())
        });
        let dev_ok = sessions.iter().find(|s| s["name"] == "dev-session").is_some_and(|s| {
            s["status"] == "Attached"
                && s["attachments"].as_array().is_some_and(|a| {
                    a.len() == 1 && a[0]["template"] == "{env}-session" && a[0]["pid"] == attach_pid
                })
        });
        Ok(prod_ok && dev_ok)
    })
    .context("attachment did not move from prod-session to dev-session")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_force_replace() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 = daemon_proc.attach("sh1", Default::default()).context("attaching tty1")?;
    let mut lm1 = tty1.line_matcher()?;
    tty1.run_cmd("echo up1")?;
    lm1.scan_until_re("up1$")?;
    let pid1 = tty1.proc.id();

    let mut tty2 = daemon_proc
        .attach("sh1", AttachArgs { force: true, ..Default::default() })
        .context("force-attaching tty2")?;
    let mut lm2 = tty2.line_matcher()?;
    tty2.run_cmd("echo up2")?;
    lm2.scan_until_re("up2$")?;
    let pid2 = tty2.proc.id();
    assert_ne!(pid1, pid2, "the two attach procs must have distinct pids");

    // The force-attach replaces tty1's attachment with tty2's; poll because the
    // detach-then-reattach the force performs is async.
    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        Ok(sessions.iter().find(|s| s["name"] == "sh1").is_some_and(|s| {
            s["status"] == "Attached"
                && s["attachments"].as_array().is_some_and(|a| a.len() == 1 && a[0]["pid"] == pid2)
        }))
    })
    .context("attachment pid should flip from tty1 to the force-attaching tty2")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_reattach_updates_pid() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
    let pid1;
    {
        let mut sess1 = daemon_proc.attach("sh1", Default::default()).context("attaching sh1")?;
        let mut lm = sess1.line_matcher()?;
        sess1.run_cmd("echo up")?;
        lm.scan_until_re("up$")?;
        pid1 = sess1.proc.id();
    } // dropping sess1 kills the attach proc

    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    // Once the drop settles, the session lingers as Disconnected with no attachment
    // (poll: the clear lands after bidi-stream-done, see the detach test).
    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        Ok(sessions.iter().find(|s| s["name"] == "sh1").is_some_and(|s| {
            s["status"] == "Disconnected"
                && s["attachments"].as_array().is_some_and(|a| a.is_empty())
        }))
    })
    .context("sh1 should be Disconnected with no attachment after the client drops")?;

    // Reattaching is a different process, so the reported pid must update.
    let mut sess2 = daemon_proc.attach("sh1", Default::default()).context("reattaching sh1")?;
    let mut lm2 = sess2.line_matcher()?;
    sess2.run_cmd("echo up2")?;
    lm2.scan_until_re("up2$")?;
    let pid2 = sess2.proc.id();
    assert_ne!(pid1, pid2, "reattach must be a distinct process");

    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        Ok(sessions.iter().find(|s| s["name"] == "sh1").is_some_and(|s| {
            s["status"] == "Attached"
                && s["attachments"].as_array().is_some_and(|a| a.len() == 1 && a[0]["pid"] == pid2)
        }))
    })
    .context("reattachment should report the new client's pid, not the stale one")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_busy_reject_preserves_incumbent() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 = daemon_proc.attach("sh1", Default::default()).context("attaching tty1")?;
    let mut lm1 = tty1.line_matcher()?;
    tty1.run_cmd("echo up1")?;
    lm1.scan_until_re("up1$")?;
    let pid1 = tty1.proc.id();

    // A second, non-force attach is rejected as busy.
    let mut tty2 = daemon_proc.attach("sh1", Default::default()).context("attaching tty2")?;
    let mut elm2 = tty2.stderr_line_matcher()?;
    elm2.scan_until_re("already has a terminal attached$")?;
    let pid2 = tty2.proc.id();

    // The rejected attach must leave the incumbent's attachment untouched.
    let sessions = list_sessions(&mut daemon_proc)?;
    let sh1 =
        sessions.iter().find(|s| s["name"] == "sh1").ok_or_else(|| anyhow!("sh1 not found"))?;
    assert_eq!(sh1["status"], "Attached");
    let attachments =
        sh1["attachments"].as_array().ok_or_else(|| anyhow!("missing attachments array"))?;
    assert_eq!(attachments.len(), 1, "busy reject must not add or remove an attachment");
    assert_eq!(attachments[0]["pid"], pid1, "incumbent attachment pid must be unchanged");
    assert_ne!(attachments[0]["pid"], pid2, "the rejected client's pid must not appear");

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_per_session_isolation() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-enter"]);

    let sess1 = daemon_proc.attach("sh1", Default::default()).context("attaching sh1")?;
    let sess2 = daemon_proc.attach("sh2", Default::default()).context("attaching sh2")?;
    waiter.wait_event("daemon-bidi-stream-enter")?;
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-enter")?);

    let pid1 = sess1.proc.id();
    let pid2 = sess2.proc.id();
    assert_ne!(pid1, pid2, "the two attach procs must have distinct pids");

    // Each session must carry its own attachment — not swapped or shared.
    let sessions = list_sessions(&mut daemon_proc)?;
    let s1 =
        sessions.iter().find(|s| s["name"] == "sh1").ok_or_else(|| anyhow!("sh1 not found"))?;
    let s2 =
        sessions.iter().find(|s| s["name"] == "sh2").ok_or_else(|| anyhow!("sh2 not found"))?;
    let a1 = s1["attachments"].as_array().ok_or_else(|| anyhow!("sh1 missing attachments"))?;
    let a2 = s2["attachments"].as_array().ok_or_else(|| anyhow!("sh2 missing attachments"))?;

    assert_eq!(a1.len(), 1);
    assert_eq!(a2.len(), 1);
    assert_eq!(a1[0]["pid"], pid1);
    assert_eq!(a2[0]["pid"], pid2);
    assert_eq!(a1[0]["template"], "sh1");
    assert_eq!(a2[0]["template"], "sh2");

    Ok(())
}

#[test]
#[timeout(30000)]
fn json_attachment_kill_removes_session() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let bidi_enter_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-enter"]);
    let _sess1 = daemon_proc.attach("sh1", Default::default()).context("attaching sh1")?;
    daemon_proc.events = Some(bidi_enter_w.wait_final_event("daemon-bidi-stream-enter")?);

    let sessions = list_sessions(&mut daemon_proc)?;
    assert!(sessions.iter().any(|s| s["name"] == "sh1"), "sh1 should be present before kill");

    let out = daemon_proc.kill(vec![String::from("sh1")])?;
    assert!(out.status.success(), "kill did not exit successfully");

    // The session and its attachment should vanish from list --json entirely.
    support::wait_until(|| {
        let sessions = list_sessions(&mut daemon_proc)?;
        Ok(!sessions.iter().any(|s| s["name"] == "sh1"))
    })
    .context("killed session should disappear from list --json")?;

    Ok(())
}
