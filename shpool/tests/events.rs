use std::{io::BufReader, os::unix::net::UnixStream};

use anyhow::{anyhow, Context};
use ntest::timeout;
use serde_json::Value;

mod support;

use crate::support::daemon::{self, AttachArgs, DaemonArgs};

fn next_event(reader: &mut BufReader<UnixStream>) -> anyhow::Result<Value> {
    let mut line = String::new();
    let n = std::io::BufRead::read_line(reader, &mut line).context("reading event line")?;
    if n == 0 {
        return Err(anyhow!("events socket closed unexpectedly"));
    }
    serde_json::from_str(&line).with_context(|| format!("parsing event JSON: {line:?}"))
}

#[test]
#[timeout(30000)]
fn lifecycle() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub = d.connect_events()?;

    // Background attach: client connects, daemon publishes created+attached;
    // the client immediately detaches, triggering the detached event.
    let _attach = d
        .attach("s1", AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() })
        .context("starting attach proc")?;

    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");
    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    let kill_out = d.kill(vec!["s1".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    assert_eq!(next_event(&mut sub)?["type"], "session.removed");

    Ok(())
}

// The explicit detach handler and the bidi-loop unwind in the attach
// worker both touch the session on a detach. If both emitted a
// `session.detached`, the duplicate would be observable as a buffered
// event. Pin exactly one detached per detach by using a kill as a
// known-next-event fence -- a duplicate would surface as the next read
// instead of `session.removed`.
#[test]
#[timeout(30000)]
fn explicit_detach_publishes_one_event() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub = d.connect_events()?;

    // Foreground attach (no `background`) keeps the session attached.
    let _attach = d
        .attach("s", AttachArgs { null_stdin: true, ..AttachArgs::default() })
        .context("starting attach proc")?;
    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");

    let detach_out = d.detach(vec!["s".into()]).context("running detach")?;
    assert!(detach_out.status.success(), "detach failed: {:?}", detach_out);

    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    // Wait for the unwind path to complete (session shows disconnected in
    // the list output) so any duplicate detached event would already be
    // queued by the time we issue the kill below.
    d.wait_until_list_matches(|out| out.contains("disconnected"))?;

    let kill_out = d.kill(vec!["s".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    let next = next_event(&mut sub)?;
    assert_eq!(
        next["type"], "session.removed",
        "expected next event to be removed, got {next} -- possible duplicate detached"
    );

    Ok(())
}

// Reattach should produce a single `session.attached` for the existing
// session, with no `session.created`. Catches regressions where the
// reattach path accidentally falls through to the create path.
#[test]
#[timeout(30000)]
fn reattach_emits_attached_only() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub = d.connect_events()?;

    let _attach1 = d
        .attach("s", AttachArgs { background: true, null_stdin: true, ..AttachArgs::default() })
        .context("first attach")?;
    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");
    // `null_stdin: true` closes the attach client's stdin immediately, so
    // the bidi loop unwinds and the session detaches without further
    // input from the test.
    assert_eq!(next_event(&mut sub)?["type"], "session.detached");

    d.wait_until_list_matches(|out| out.contains("disconnected"))?;

    let _attach2 = d
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

// Force-reattach (`shpool attach -f`) onto a session that already has a client
// attached: the daemon kicks the old client and the new one takes over the
// *same* subshell. Exactly one `session.detached` (the kick) and one
// `session.attached` (the takeover) should fire, with no
// `session.created`/`session.removed` -- the subshell process survives the
// whole handover. A kill fence pins this: with the subshell still alive, the
// known next event is `session.removed`, so a duplicate detached on the kick or
// a stray create/remove would surface there instead.
#[test]
#[timeout(30000)]
fn force_reattach_kicks_old_client_and_keeps_subshell() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub = d.connect_events()?;

    // Foreground attach (no `background`) keeps the client attached and holding the
    // session's inner lock, so the forced attach below actually hits the busy path.
    let _attach1 = d
        .attach("s", AttachArgs { null_stdin: true, ..AttachArgs::default() })
        .context("first attach")?;
    assert_eq!(next_event(&mut sub)?["type"], "session.created");
    assert_eq!(next_event(&mut sub)?["type"], "session.attached");

    // Ensure attach1 is fully attached before forcing; otherwise attach2 could win
    // the create/reattach race without a kick and no `session.detached` would fire.
    d.wait_until_list_matches(|out| out.contains("attached"))?;

    let _attach2 = d
        .attach("s", AttachArgs { force: true, null_stdin: true, ..AttachArgs::default() })
        .context("forced reattach")?;

    assert_eq!(
        next_event(&mut sub)?["type"],
        "session.detached",
        "expected the old client to be kicked"
    );
    let attached = next_event(&mut sub)?;
    assert_eq!(
        attached["type"], "session.attached",
        "expected attached on forced reattach, got {attached}"
    );

    // Sync so any late detached from the kicked client's unwind would already be
    // queued, then fence with a kill: the subshell is still alive, so the next
    // event must be exactly `session.removed`.
    d.wait_until_list_matches(|out| out.contains("attached"))?;

    let kill_out = d.kill(vec!["s".into()]).context("running kill")?;
    assert!(kill_out.status.success(), "kill failed: {:?}", kill_out);

    let next = next_event(&mut sub)?;
    assert_eq!(
        next["type"], "session.removed",
        "expected next event to be removed, got {next} -- possible duplicate \
         detached or stray create/remove on the force-reattach path"
    );

    Ok(())
}

// SIGTERM should clean up both sockets via the signal handler, since
// process::exit bypasses any RAII guard.
#[test]
#[timeout(30000)]
fn signal_exit_unlinks_sockets() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let main_sock = d.socket_path.clone();
    let events_sock = d.events_socket_path();
    assert!(main_sock.exists(), "main socket should exist while daemon runs");
    assert!(events_sock.exists(), "events socket should exist while daemon runs");

    let pid =
        nix::unistd::Pid::from_raw(d.proc.as_ref().expect("daemon process handle").id() as i32);
    nix::sys::signal::kill(pid, nix::sys::signal::SIGTERM).context("sending SIGTERM")?;
    d.proc_wait().context("waiting for daemon to exit")?;

    assert!(!main_sock.exists(), "main socket should be unlinked on signal exit");
    assert!(!events_sock.exists(), "events socket should be unlinked on signal exit");

    Ok(())
}

#[test]
#[timeout(30000)]
fn multiple_subscribers_each_get_independent_streams() -> anyhow::Result<()> {
    let mut d = daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut sub_a = d.connect_events()?;
    let mut sub_b = d.connect_events()?;

    let _attach = d
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
