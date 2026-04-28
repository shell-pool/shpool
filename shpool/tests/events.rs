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
fn lifecycle() -> anyhow::Result<()> {
    let mut daemon =
        Proc::new("norc.toml", DaemonArgs { listen_events: false, ..DaemonArgs::default() })
            .context("starting daemon proc")?;
    let mut sub = connect_events(&daemon)?;

    // Background attach: client connects, daemon publishes created+attached;
    // the client immediately detaches, triggering the detached event.
    let _attach = daemon
        .attach("s1", AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() })
        .context("starting attach proc")?;

    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");
    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    let kill_out = daemon.kill(vec!["s1".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    assert_eq!(next_event(&mut sub)?["type"], "session.removed");

    Ok(())
}

// `shpool detach` triggers two code paths that both updated the session:
// the explicit handler and, asynchronously, the bidi-loop unwind in the
// attach worker. An earlier version emitted SessionDetached from both,
// producing a duplicate event. This test pins exactly one detached event
// per detach by using a kill as a known-next-event fence — if a duplicate
// detached were buffered, the next read would return it instead of
// `session.removed`.
#[test]
#[timeout(30000)]
fn explicit_detach_publishes_one_event() -> anyhow::Result<()> {
    let mut daemon =
        Proc::new("norc.toml", DaemonArgs { listen_events: false, ..DaemonArgs::default() })
            .context("starting daemon proc")?;
    let mut sub = connect_events(&daemon)?;

    // Foreground attach (no `background`) keeps the session attached.
    let _attach = daemon
        .attach("s", AttachArgs { null_stdin: true, ..AttachArgs::default() })
        .context("starting attach proc")?;
    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");

    let detach_out = daemon.detach(vec!["s".into()]).context("running detach")?;
    assert!(detach_out.status.success(), "detach failed: {:?}", detach_out);

    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    // Wait for the unwind path to complete (session shows disconnected in
    // the list output) so any duplicate detached event would already be
    // queued by the time we issue the kill below.
    daemon.wait_until_list_matches(|out| out.contains("disconnected"))?;

    let kill_out = daemon.kill(vec!["s".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    let next = next_event(&mut sub)?;
    assert_eq!(
        next["type"], "session.removed",
        "expected next event to be removed, got {next} — possible duplicate detached"
    );

    Ok(())
}

// Reattach should produce a single `session.attached` for the existing
// session, with no `session.created`. Catches regressions where the
// reattach path accidentally falls through to the create path.
#[test]
#[timeout(30000)]
fn reattach_emits_attached_only() -> anyhow::Result<()> {
    let mut daemon =
        Proc::new("norc.toml", DaemonArgs { listen_events: false, ..DaemonArgs::default() })
            .context("starting daemon proc")?;
    let mut sub = connect_events(&daemon)?;

    let _attach1 = daemon
        .attach("s", AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() })
        .context("first attach")?;
    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");
    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    daemon.wait_until_list_matches(|out| out.contains("disconnected"))?;

    let _attach2 = daemon
        .attach("s", AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() })
        .context("reattach")?;

    let attached = next_event(&mut sub)?;
    assert_eq!(
        attached["type"], "session.attached",
        "expected attached on reattach, got {attached}"
    );
    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    Ok(())
}

// SIGTERM should clean up both sockets via the signal handler, since
// process::exit bypasses any RAII guard.
#[test]
#[timeout(30000)]
fn signal_exit_unlinks_sockets() -> anyhow::Result<()> {
    let mut daemon =
        Proc::new("norc.toml", DaemonArgs { listen_events: false, ..DaemonArgs::default() })
            .context("starting daemon proc")?;
    let main_sock = daemon.socket_path.clone();
    let events_sock = events_socket_path(&daemon);
    assert!(main_sock.exists(), "main socket should exist while daemon runs");
    assert!(events_sock.exists(), "events socket should exist while daemon runs");

    let pid = nix::unistd::Pid::from_raw(
        daemon.proc.as_ref().expect("daemon process handle").id() as i32,
    );
    nix::sys::signal::kill(pid, nix::sys::signal::SIGTERM).context("sending SIGTERM")?;
    daemon.proc_wait().context("waiting for daemon to exit")?;

    assert!(!main_sock.exists(), "main socket should be unlinked on signal exit");
    assert!(!events_sock.exists(), "events socket should be unlinked on signal exit");

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_subscribers_each_get_independent_streams() -> anyhow::Result<()> {
    let mut daemon =
        Proc::new("norc.toml", DaemonArgs { listen_events: false, ..DaemonArgs::default() })
            .context("starting daemon proc")?;
    let mut sub_a = connect_events(&daemon)?;
    let mut sub_b = connect_events(&daemon)?;

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
