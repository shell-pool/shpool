use anyhow::Context;

mod support;

// the happy path for attaching
#[test]
fn attach_test() -> anyhow::Result<()> {
    let mut daemon_proc = support::DaemonProc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("echo hi")?;
    line_matcher.match_re("echo hi$")?;
    line_matcher.match_re("hi$")?;

    /* TODO(ethan): sometimes the next line is 'hi' instead of 'echo ping'
     *
     * It seems to be due to the fact that the child shell sometimes gives
     * us back
     * ```
     * echo hi
     * hi
     * echo ping
     * ping
     * ```
     *
     * and sometimes gives us back
     * ```
     * echo hi
     * prompt> echo hi
     * hi
     * ```
     *
     * I don't understand why we sometimes get a prompt/command back and
     * sometimes don't. It seems like we should either always or never get
     * one.
    attach_proc.run_cmd("echo ping")?;
    line_matcher.match_re("echo ping$")?;
    line_matcher.match_re("ping$")?;
    */

    Ok(())
}
