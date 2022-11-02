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
    line_matcher.match_re("hi$")?;

    attach_proc.run_cmd("echo ping")?;
    line_matcher.match_re("ping$")?;

    Ok(())
}
