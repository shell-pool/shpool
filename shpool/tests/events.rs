use std::{
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

use anyhow::{anyhow, Context};
use ntest::timeout;
use serde_json::Value;

mod support;

use crate::support::daemon::{AttachArgs, DaemonArgs, Proc};

fn events_socket_path(daemon: &Proc) -> PathBuf {
    daemon.socket_path.with_file_name("events.socket")
}

fn connect_events(daemon: &Proc) -> anyhow::Result<BufReader<UnixStream>> {
    let path = events_socket_path(daemon);
    let mut sleep_dur = Duration::from_millis(5);
    for _ in 0..12 {
        if let Ok(stream) = UnixStream::connect(&path) {
            return Ok(BufReader::new(stream));
        }
        std::thread::sleep(sleep_dur);
        sleep_dur *= 2;
    }
    Err(anyhow!("events socket never became available at {:?}", path))
}

fn next_event(reader: &mut BufReader<UnixStream>) -> anyhow::Result<Value> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).context("reading event line")?;
    if n == 0 {
        return Err(anyhow!("events socket closed unexpectedly"));
    }
    serde_json::from_str(&line).with_context(|| format!("parsing event JSON: {line:?}"))
}

#[test]
#[timeout(30000)]
fn snapshot_then_lifecycle() -> anyhow::Result<()> {
    let mut daemon = Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub = connect_events(&daemon)?;

    let snap = next_event(&mut sub)?;
    assert_eq!(snap["type"], "snapshot");
    assert_eq!(snap["sessions"].as_array().unwrap().len(), 0);

    // Background attach: client connects, daemon publishes created+attached;
    // the client immediately detaches, triggering the detached event.
    let _attach = daemon
        .attach(
            "s1",
            AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() },
        )
        .context("starting attach proc")?;

    let created = next_event(&mut sub)?;
    assert_eq!(created["type"], "session.created");
    assert_eq!(created["name"], "s1");
    assert!(created["started_at_unix_ms"].is_number());

    let attached = next_event(&mut sub)?;
    assert_eq!(attached["type"], "session.attached");
    assert_eq!(attached["name"], "s1");

    let detached = next_event(&mut sub)?;
    assert_eq!(detached["type"], "session.detached");
    assert_eq!(detached["name"], "s1");

    let kill_out = daemon.kill(vec!["s1".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    let removed = next_event(&mut sub)?;
    assert_eq!(removed["type"], "session.removed");
    assert_eq!(removed["name"], "s1");
    assert_eq!(removed["reason"], "killed");

    Ok(())
}

#[test]
#[timeout(30000)]
fn snapshot_includes_existing_sessions() -> anyhow::Result<()> {
    let mut daemon = Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let _attach = daemon
        .attach(
            "pre-existing",
            AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() },
        )
        .context("starting attach proc")?;

    // Wait for the session to land in the table before subscribing.
    daemon.wait_until_list_matches(|out| out.contains("pre-existing"))?;

    let mut sub = connect_events(&daemon)?;
    let snap = next_event(&mut sub)?;
    assert_eq!(snap["type"], "snapshot");
    let sessions = snap["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["name"], "pre-existing");

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_subscribers_each_get_independent_streams() -> anyhow::Result<()> {
    let mut daemon = Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub_a = connect_events(&daemon)?;
    let mut sub_b = connect_events(&daemon)?;

    assert_eq!(next_event(&mut sub_a)?["type"], "snapshot");
    assert_eq!(next_event(&mut sub_b)?["type"], "snapshot");

    let _attach = daemon
        .attach(
            "shared",
            AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() },
        )
        .context("starting attach proc")?;

    for sub in [&mut sub_a, &mut sub_b] {
        assert_eq!(next_event(sub)?["type"], "session.created");
        assert_eq!(next_event(sub)?["type"], "session.attached");
        assert_eq!(next_event(sub)?["type"], "session.detached");
    }

    Ok(())
}
