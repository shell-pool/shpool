use std::{
    thread,
    time,
};

use anyhow::Context;

mod support;

/* broken. TODO: re-enable
#[test]
fn happy_path_set_meta_first() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let _set_metadata_proc = daemon_proc.ssh_set_metadata("/dev/pts/2")
        .context("setting metadata for attach")?;

    let mut remote_cmd_proc = daemon_proc.ssh_remote_cmd()
        .context("spawning ssh remote cmd")?;

    let mut line_matcher = remote_cmd_proc.line_matcher()?;

    remote_cmd_proc.run_cmd("echo hi")?;
    line_matcher.match_re("hi$")?;

    Ok(())
}
*/

/* broken. TODO: re-enable
#[test]
fn happy_path_remote_cmd_first() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut remote_cmd_proc = daemon_proc.ssh_remote_cmd()
        .context("spawning ssh remote cmd")?;

    let _set_metadata_proc = daemon_proc.ssh_set_metadata("/dev/pts/2")
        .context("setting metadata for attach")?;

    let mut line_matcher = remote_cmd_proc.line_matcher()?;

    remote_cmd_proc.run_cmd("echo hi")?;
    line_matcher.match_re("hi$")?;

    Ok(())
}
*/

#[test]
fn remote_cmd_timeout() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("short_ssh_timeout.toml").context("starting daemon proc")?;

    let mut remote_cmd_proc = daemon_proc
        .ssh_remote_cmd()
        .context("spawning ssh remote cmd")?;

    thread::sleep(time::Duration::from_millis(500));

    let mut line_matcher = remote_cmd_proc.line_matcher()?;
    line_matcher.match_re("timeout$")?;

    Ok(())
}

/* broken. TODO: re-enable
#[test]
fn set_metadata_timeout() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("short_ssh_timeout.toml")
        .context("starting daemon proc")?;

    let mut timedout_metadata_proc = daemon_proc.ssh_set_metadata("first")
        .context("setting metadata that will get discarded")?;

    thread::sleep(time::Duration::from_millis(500));

    let mut line_matcher = timedout_metadata_proc.line_matcher()?;
    line_matcher.match_re("timeout$")?;

    Ok(())
}
*/

/* broken. TODO: re-enable
#[test]
fn handshake_after_timedout_set_metadata() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("short_ssh_timeout.toml")
        .context("starting daemon proc")?;

    let _timedout_metadata_proc = daemon_proc.ssh_set_metadata("first")
        .context("setting metadata that will get discarded")?;

    // timeout is 100 ms, so this should invalidate the metadata
    thread::sleep(time::Duration::from_millis(200));

    let mut remote_cmd_proc = daemon_proc.ssh_remote_cmd()
        .context("spawning ssh remote cmd")?;
    let _metadata_proc = daemon_proc.ssh_set_metadata("second")
        .context("setting metadata that should get used")?;

    let mut line_matcher = remote_cmd_proc.line_matcher()?;
    remote_cmd_proc.run_cmd("echo $SHPOOL_SESSION_NAME")?;
    line_matcher.match_re("second$")?;

    Ok(())
}
*/
