use std::time;

use anyhow::Context;

mod support;

#[test]
fn happy_path() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

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

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=1")?;
        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    } // falling out of scope kills attach_proc

    // sleep for at least the heartbeat duration so the daemon has a
    // chance to detect the broken pipe
    std::thread::sleep(time::Duration::from_millis(600));

    {
        let mut attach_proc = daemon_proc.attach("sh1")
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    }

    Ok(())
}
