use anyhow::Context;

mod support;

#[test]
fn happy_path() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    // not really needed, just here to test the events system
    attach_proc.await_event("attach-startup")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // not really needed, just here to test the events system
    daemon_proc.await_event("daemon-about-to-listen")?;

    attach_proc.run_cmd("echo hi")?;
    line_matcher.match_re("hi$")?;

    attach_proc.run_cmd("echo ping")?;
    line_matcher.match_re("ping$")?;

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
fn bounce() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=1")?;
        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    } // falling out of scope kills attach_proc

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    }

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
fn explicit_exit() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=first")?;
        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("first$")?;

        attach_proc.run_cmd("exit")?;

        // wait until the daemon has cleaned up before dropping
        // and explicitly killing the attach proc.
        daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);
    }

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ${MYVAR:-second}")?;
        line_matcher.match_re("second$")?;
    }

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
fn exit_immediate_drop() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;

    let reap_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=first")?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("first$")?;

        attach_proc.run_cmd("exit")?;
        line_matcher.match_re("exit$")?;

        // Immediately kill the attach proc after we've written exit
        // to bring the connection down.
    }

    daemon_proc.events = Some(reap_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ${MYVAR:-second}")?;
        line_matcher.match_re("second$")?;
    }

    Ok(())
}

/*
#[test]
fn up_arrow_no_crash() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // For some reason this makes bash crash. No idea why.
    attach_proc.run_raw_cmd(vec![27, 91, 65, 10])?; // up arrow

    attach_proc.run_cmd("echo ping")?;
    line_matcher.match_re("ping$")?;

    Ok(())
}
*/
