#![allow(clippy::literal_string_with_formatting_args)]

use std::time;

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::{AttachArgs, DaemonArgs};

/// The (name, id, status) of every session, from `shpool list --json`.
fn sessions(daemon_proc: &mut support::daemon::Proc) -> anyhow::Result<Vec<(String, u64, String)>> {
    let out = daemon_proc.list_json()?;
    assert!(out.status.success(), "list --json failed");
    let list: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let mut sessions: Vec<_> = list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_string(),
                s["id"].as_u64().unwrap(),
                s["status"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    sessions.sort();
    Ok(sessions)
}

#[test]
#[timeout(30000)]
fn detached() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let _attach_proc = daemon_proc
        .attach("sh1", AttachArgs { background: true, ..Default::default() })
        .context("starting attach proc")?;
    daemon_proc.wait_until_list_matches(|out| out.contains("sh1"))?;
    let before = sessions(&mut daemon_proc)?;

    let out = daemon_proc.rename(&["sh1", "sh2"])?;
    assert!(out.status.success(), "rename failed: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());

    let after = sessions(&mut daemon_proc)?;
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].0, "sh2");
    // the id stays the same across the rename
    assert_eq!(after[0].1, before[0].1);

    Ok(())
}

#[test]
#[timeout(30000)]
fn errors() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_procs = vec![];
    for name in ["sh1", "sh2"] {
        attach_procs.push(
            daemon_proc
                .attach(name, AttachArgs { background: true, ..Default::default() })
                .context("starting attach proc")?,
        );
    }
    daemon_proc.wait_until_list_matches(|out| out.contains("sh1") && out.contains("sh2"))?;

    let out = daemon_proc.rename(&["nope", "sh3"])?;
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not found: nope"));

    let out = daemon_proc.rename(&["sh1", "sh2"])?;
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists: sh2"));

    for bad in ["has space", "has/slash", ".", ".."] {
        let out = daemon_proc.rename(&["sh1", bad])?;
        assert!(!out.status.success(), "renamed to {bad:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("invalid session name"));
    }

    // renaming a session to its own name is a no-op
    let out = daemon_proc.rename(&["sh1", "sh1"])?;
    assert!(out.status.success());

    let names: Vec<_> = sessions(&mut daemon_proc)?.into_iter().map(|s| s.0).collect();
    assert_eq!(names, vec!["sh1", "sh2"]);

    Ok(())
}

// A rename shows up in an attached shell's prompt and $SHPOOL_SESSION_NAME,
// and the attach process keeps track of its session: resizes still reach it,
// and when the shell exits the renamed entry is removed.
#[test]
#[timeout(30000)]
fn attached() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("prompt_prefix_bash.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

    let mut attach_proc = daemon_proc.attach("sh1", AttachArgs::default())?;
    let mut lm = attach_proc.line_matcher()?;
    waiter.wait_event("daemon-wrote-s2c-chunk")?;
    attach_proc.run_cmd("echo")?;
    lm.scan_until_re(".*session_name=sh1 prompt>.*")?;

    let out = daemon_proc.rename(&["sh1", "renamed"])?;
    assert!(out.status.success(), "rename failed: {}", String::from_utf8_lossy(&out.stderr));

    // A prompt has no trailing newline, so the next command's output ends up
    // on the same line as the first prompt drawn after the rename.
    attach_proc.run_cmd("echo")?;
    attach_proc.run_cmd("echo name=$SHPOOL_SESSION_NAME")?;
    lm.scan_until_re("session_name=renamed prompt> name=renamed$")?;

    let after = sessions(&mut daemon_proc)?;
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].0, "renamed");
    assert_eq!(after[0].2, "Attached");

    attach_proc.run_cmd("exit")?;
    attach_proc.proc.wait()?;
    daemon_proc.wait_until_list_matches(|out| !out.contains("renamed"))?;

    Ok(())
}

// After a rename, a new session can take the old name, and the two
// don't share a session directory.
#[test]
#[timeout(30000)]
fn reuse_old_name() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut first = daemon_proc.attach("sh1", AttachArgs::default())?;
    let mut first_lm = first.line_matcher()?;
    first.run_cmd("echo dir=$SHPOOL_SESSION_DIR")?;
    let first_dir = first_lm.scan_until_re_captures("dir=(.*)$")?[1].clone().unwrap();

    let out = daemon_proc.rename(&["sh1", "sh2"])?;
    assert!(out.status.success(), "rename failed: {}", String::from_utf8_lossy(&out.stderr));

    let mut second = daemon_proc.attach("sh1", AttachArgs::default())?;
    let mut second_lm = second.line_matcher()?;
    second.run_cmd("echo dir=$SHPOOL_SESSION_DIR")?;
    let second_dir = second_lm.scan_until_re_captures("dir=(.*)$")?[1].clone().unwrap();
    assert_ne!(first_dir, second_dir);

    let names: Vec<_> = sessions(&mut daemon_proc)?.into_iter().map(|s| (s.0, s.2)).collect();
    assert_eq!(
        names,
        vec![
            (String::from("sh1"), String::from("Attached")),
            (String::from("sh2"), String::from("Attached")),
        ]
    );

    // Exiting the renamed shell removes it, not the new sh1.
    first.run_cmd("exit")?;
    first.proc.wait()?;
    daemon_proc.wait_until_list_matches(|out| !out.contains("sh2"))?;
    let names: Vec<_> = sessions(&mut daemon_proc)?.into_iter().map(|s| s.0).collect();
    assert_eq!(names, vec!["sh1"]);

    Ok(())
}

// From inside a session, `shpool rename NEW` renames the current session,
// and `shpool detach` finds the session by its id even though
// $SHPOOL_SESSION_NAME is stale until the next prompt.
#[test]
#[timeout(30000)]
fn current_session() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-bidi-stream-enter", "daemon-bidi-stream-done"]);

    let mut attach_proc = daemon_proc.attach("sh1", AttachArgs::default())?;
    waiter.wait_event("daemon-bidi-stream-enter")?;

    let bin = support::shpool_bin()?;
    let socket = daemon_proc.socket_path.clone();
    attach_proc.run_cmd(&format!(
        "'{bin}' --socket '{socket}' rename inner && '{bin}' --socket '{socket}' detach",
        bin = bin.display(),
        socket = socket.display(),
    ))?;

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);
    assert!(attach_proc.proc.wait()?.success());

    let names: Vec<_> = sessions(&mut daemon_proc)?.into_iter().map(|s| (s.0, s.2)).collect();
    assert_eq!(names, vec![(String::from("inner"), String::from("Disconnected"))]);

    Ok(())
}

// The ttl reaper keeps track of a session across a rename, and does not
// reap a new session that takes the old name.
#[test]
#[timeout(30000)]
fn ttl() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        // Long enough that the rename below lands before the reaper fires,
        // even on a slow CI runner.
        .attach("sh1", AttachArgs { ttl: Some(time::Duration::from_secs(3)), ..Default::default() })
        .context("starting attach proc")?;
    let mut lm = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo hi")?;
    lm.scan_until_re("hi$")?;

    let out = daemon_proc.rename(&["sh1", "sh2"])?;
    assert!(out.status.success(), "rename failed: {}", String::from_utf8_lossy(&out.stderr));
    let _new_sh1 = daemon_proc
        .attach("sh1", AttachArgs { background: true, ..Default::default() })
        .context("starting attach proc")?;
    daemon_proc.wait_until_list_matches(|out| out.contains("sh1"))?;

    // wait for the reaper to clobber the renamed session
    daemon_proc.wait_until_list_matches(|out| !out.contains("sh2"))?;

    let names: Vec<_> = sessions(&mut daemon_proc)?.into_iter().map(|s| s.0).collect();
    assert_eq!(names, vec!["sh1"]);

    Ok(())
}
