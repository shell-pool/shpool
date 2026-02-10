#![allow(clippy::literal_string_with_formatting_args)]

use anyhow::Context;
use ntest::timeout;

mod support;

use crate::support::daemon::DaemonArgs;

#[test]
#[timeout(30000)]
fn screen_restore() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("vterm_screen.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo foo")?;
        line_matcher.scan_until_re("foo$")?;
    }

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        // the re-attach should redraw the screen for us, so we should
        // get a line with "foo" as part of the re-drawn screen.
        line_matcher.scan_until_re("foo$")?;

        attach_proc.proc.kill()?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn lines_restore() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("vterm_lines.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo foo")?;
        line_matcher.scan_until_re("foo$")?;
    }

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        // the re-attach should redraw the last 2 lines for us, so we should
        // get a line with "foo" as part of the re-drawn screen.
        line_matcher.scan_until_re("foo$")?;
    }

    Ok(())
}
