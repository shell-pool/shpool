use std::{env, fs, time, io::Read};

use anyhow::Context;

mod support;

#[test]
fn happy_path() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1", vec![])
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

    // make sure that shpool sets a $USER variable
    attach_proc.run_cmd(r#"echo "user=$USER" "#)?;
    line_matcher.match_re("user=[a-zA-Z0-9]+$")?;

    Ok(())
}

#[test]
fn symlink_ssh_auth_sock() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let tmp_dir = daemon_proc.tmp_dir.as_ref().unwrap();
    let fake_auth_sock_tgt = tmp_dir.path().join("ssh-auth-sock-target.fake");
    fs::File::create(&fake_auth_sock_tgt)?;

    let mut attach_proc = daemon_proc.attach(
        "sh1",
        vec![
            (
                String::from("SSH_AUTH_SOCK"),
                String::from(fake_auth_sock_tgt.to_str().unwrap()),
            )
        ],
    ).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
    line_matcher.match_re(r#".*sh1/ssh-auth-sock.socket ->.*ssh-auth-sock-target.fake$"#)?;

    Ok(())
}

#[test]
fn config_disable_symlink_ssh_auth_sock() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("disable_symlink_ssh_auth_sock.toml")
        .context("starting daemon proc")?;

    let tmp_dir = daemon_proc.tmp_dir.as_ref().unwrap();
    let fake_auth_sock_tgt = tmp_dir.path().join("ssh-auth-sock-target.fake");
    fs::File::create(&fake_auth_sock_tgt)?;

    let mut attach_proc = daemon_proc.attach(
        "sh1",
        vec![
            (
                String::from("SSH_AUTH_SOCK"),
                String::from(fake_auth_sock_tgt.to_str().unwrap()),
            )
        ],
    ).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
    line_matcher.match_re(r#".*No such file or directory$"#)?;

    Ok(())
}

#[test]
fn forward_client_var() -> anyhow::Result<()> {
    env::set_var("MY_FORWARD_VAR", "forward-var-value");

    let mut daemon_proc = support::daemon::Proc::new("forward_client_env.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1", vec![])
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("echo $MY_FORWARD_VAR")?;
    line_matcher.match_re("forward-var-value$")?;

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
fn bounce() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
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
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    }

    Ok(())
}

#[test]
fn two_at_once() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let mut attach_proc1 = daemon_proc.attach("sh1", vec![])
        .context("starting sh1")?;
    let mut attach_proc2 = daemon_proc.attach("sh2", vec![])
        .context("starting sh2")?;

    let mut line_matcher1 = attach_proc1.line_matcher()?;
    let mut line_matcher2 = attach_proc2.line_matcher()?;

    attach_proc1.run_cmd("echo proc1").context("proc1 echo")?;
    line_matcher1.match_re("proc1$").context("proc1 match")?;

    attach_proc2.run_cmd("echo proc2").context("proc2 echo")?;
    line_matcher2.match_re("proc2$").context("proc2 match")?;

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
fn explicit_exit() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
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
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
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
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;

    let reap_w = daemon_proc.events.take().unwrap()
        .waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=first")?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("first$")?;

        attach_proc.run_cmd("exit")?;
        line_matcher.match_re("logout$")?;

        // Immediately kill the attach proc after we've written exit
        // to bring the connection down.
    }

    daemon_proc.events = Some(reap_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc = daemon_proc.attach("sh1", vec![])
            .context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ${MYVAR:-second}")?;
        line_matcher.match_re("second$")
            .context("matching second")?;
    }

    Ok(())
}

#[test]
fn output_flood() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1", vec![])
        .context("starting attach proc")?;

    attach_proc.run_cmd("cat /dev/urandom | hexdump")?;

    let flood_duration = time::Duration::from_secs(2);
    let start_time = time::Instant::now();
    let mut stdout = attach_proc.proc.stdout.take().unwrap();
    let mut buf: [u8; 1024*256] = [0; 1024*256];
    while time::Instant::now().duration_since(start_time) < flood_duration {
        stdout.read(&mut buf).context("reading a chunk of flood output")?;
    }

    Ok(())
}

/* flaky. TODO: fix
#[test]
fn up_arrow_no_crash() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml")
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc.attach("sh1")
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // Before we put the pty into raw mode, this would
    // cause crashes.
    attach_proc.run_raw_cmd(vec![27, 91, 65, 10])?; // up arrow
    line_matcher.match_re("logout$")?;

    Ok(())
}
*/
