#![allow(clippy::literal_string_with_formatting_args)]

use std::{
    ffi::OsStr,
    fs,
    io::BufRead,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread, time,
};

use anyhow::{anyhow, Context};
use ntest::timeout;
use regex::Regex;

mod support;

use crate::support::{
    daemon::{AttachArgs, DaemonArgs},
    tmpdir,
};

#[test]
#[timeout(30000)]
fn happy_path() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // not really needed, just here to test the events system
    attach_proc.await_event("attach-startup")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // not really needed, just here to test the events system
    daemon_proc.await_event("daemon-about-to-listen")?;

    attach_proc.run_cmd("echo hi")?;
    line_matcher.scan_until_re("hi$")?;

    attach_proc.run_cmd("echo ping")?;
    line_matcher.match_re("ping$")?;

    // make sure that shpool sets a $USER variable
    attach_proc.run_cmd(r#"echo "user=$USER" "#)?;
    line_matcher.match_re("user=[a-zA-Z0-9]+$")?;

    // make sure that shpool sets the $SHELL variable
    attach_proc.run_cmd(r#"echo "shell=$SHELL" "#)?;
    line_matcher.match_re("shell=[/a-zA-Z0-9]+$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn custom_cmd() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let script = support::testdata_file("echo_stop.sh");
    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs {
                cmd: Some(format!("{} foo", script.into_os_string().into_string().unwrap())),
                ..Default::default()
            },
        )
        .context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;

    // the script first echos the arg we gave it
    line_matcher.match_re("foo$")?;

    // then it echos its argv[0] so we can make sure it has not been re-written
    // to '-echo_stop.sh' like it would for a login shell
    line_matcher.match_re(r#"\/echo_stop\.sh$"#)?;

    // then waits until we tell it to bail (so we can avoid sleeps)
    attach_proc.run_cmd("stop")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn forward_env() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("forward_env.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs {
                    extra_env: vec![
                        (String::from("FOO"), String::from("foo")), // forwarded
                        (String::from("BAR"), String::from("bar")), // forwarded
                        (String::from("BAZ"), String::from("baz")), // not forwarded
                    ],
                    ..Default::default()
                },
            )
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd(r#"echo "$FOO:$BAR:$BAZ" "#)?;
        line_matcher.scan_until_re("foo:bar:$")?;
    }

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs {
                    extra_env: vec![
                        (String::from("FOO"), String::from("foonew")), // forwarded
                        (String::from("BAR"), String::from("bar")),    // forwarded
                        (String::from("BAZ"), String::from("baz")),    // not forwarded
                    ],
                    ..Default::default()
                },
            )
            .context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd(r#"source $SHPOOL_SESSION_DIR/forward.env "#)?;
        attach_proc.run_cmd(r#"echo "$FOO:$BAR:$BAZ" "#)?;
        line_matcher.scan_until_re("foonew:bar:$")?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn symlink_ssh_auth_sock() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

    let fake_auth_sock_tgt = daemon_proc.tmp_dir.path().join("ssh-auth-sock-target.fake");
    fs::File::create(&fake_auth_sock_tgt)?;

    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs {
                extra_env: vec![(
                    String::from("SSH_AUTH_SOCK"),
                    String::from(fake_auth_sock_tgt.to_str().unwrap()),
                )],
                ..Default::default()
            },
        )
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    waiter.wait_event("daemon-wrote-s2c-chunk")?; // resize prompt redraw
    attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
    line_matcher.scan_until_re(r#".*sh1/ssh-auth-sock.socket ->.*ssh-auth-sock-target.fake$"#)?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn missing_ssh_auth_sock() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

    let fake_auth_sock_tgt = daemon_proc.tmp_dir.path().join("ssh-auth-sock-target.fake");
    fs::File::create(fake_auth_sock_tgt)?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    waiter.wait_event("daemon-wrote-s2c-chunk")?; // resize prompt re-draw
    attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
    line_matcher.scan_until_re(r#".*No such file or directory$"#)?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn fresh_shell_draws_prompt() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut reader =
        std::io::BufReader::new(attach_proc.proc.stdout.take().ok_or(anyhow!("missing stdout"))?);

    let mut output = vec![];
    reader.read_until(b'>', &mut output)?;
    let chunk = String::from_utf8_lossy(&output[..]);
    assert!(chunk.contains("prompt"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn config_disable_symlink_ssh_auth_sock() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("disable_symlink_ssh_auth_sock.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

    let fake_auth_sock_tgt = daemon_proc.tmp_dir.path().join("ssh-auth-sock-target.fake");
    fs::File::create(&fake_auth_sock_tgt)?;

    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs {
                extra_env: vec![(
                    String::from("SSH_AUTH_SOCK"),
                    String::from(fake_auth_sock_tgt.to_str().unwrap()),
                )],
                ..Default::default()
            },
        )
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    waiter.wait_event("daemon-wrote-s2c-chunk")?; // resize prompt re-draw
    attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
    line_matcher.scan_until_re(r#".*No such file or directory$"#)?;

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
#[timeout(30000)]
fn bounce() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=1")?;
        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.scan_until_re("1$")?;
    } // falling out of scope kills attach_proc

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.match_re("1$")?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn two_at_once() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc1 = daemon_proc.attach("sh1", Default::default()).context("starting sh1")?;
    let mut line_matcher1 = attach_proc1.line_matcher()?;

    let mut attach_proc2 = daemon_proc.attach("sh2", Default::default()).context("starting sh2")?;
    let mut line_matcher2 = attach_proc2.line_matcher()?;

    attach_proc1.run_cmd("echo proc1").context("proc1 echo")?;
    line_matcher1.scan_until_re("proc1$").context("proc1 match")?;

    attach_proc2.run_cmd("echo proc2").context("proc2 echo")?;
    line_matcher2.scan_until_re("proc2$").context("proc2 match")?;

    Ok(())
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
#[timeout(30000)]
fn explicit_exit() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=first")?;
        attach_proc.run_cmd("echo $MYVAR")?;
        line_matcher.scan_until_re("first$")?;

        attach_proc.run_cmd("exit")?;

        // wait until the daemon has cleaned up before dropping
        // and explicitly killing the attach proc.
        daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);
    }

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ${MYVAR:-second}")?;
        line_matcher.scan_until_re("second$")?;
    }

    Ok(())
}

// Test the attach process getting killed, then re-attaching to the
// same shell session.
#[ignore] // this test is flaky in ci. TODO: re-enable
#[test]
#[timeout(30000)]
fn exit_immediate_drop() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut waiter = daemon_proc.events.take().unwrap().waiter([
        "daemon-read-c2s-chunk",
        "daemon-read-c2s-chunk",
        "daemon-wrote-s2c-chunk",
        "daemon-read-c2s-chunk",
        "daemon-wrote-s2c-chunk",
        "daemon-bidi-stream-done",
    ]);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("export MYVAR=first")?;
        waiter.wait_event("daemon-read-c2s-chunk")?;

        attach_proc.run_cmd("echo $MYVAR")?;
        waiter.wait_event("daemon-read-c2s-chunk")?;
        line_matcher.match_re("first$")?;
        waiter.wait_event("daemon-wrote-s2c-chunk")?;

        attach_proc.run_cmd("exit")?;
        waiter.wait_event("daemon-read-c2s-chunk")?;
        line_matcher.match_re("logout$")?;
        waiter.wait_event("daemon-wrote-s2c-chunk")?;

        // Immediately kill the attach proc after we've written exit
        // to bring the connection down.
    }

    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("reattaching")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ${MYVAR:-second}")?;
        line_matcher.match_re("second$").context("matching second")?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn output_flood() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    attach_proc.run_cmd("cat /dev/urandom | hexdump")?;

    let flood_duration = time::Duration::from_secs(2);
    let start_time = time::Instant::now();
    let mut stdout = attach_proc.proc.stdout.take().unwrap();
    let mut buf: [u8; 1024 * 256] = [0; 1024 * 256];
    while time::Instant::now().duration_since(start_time) < flood_duration {
        stdout.read(&mut buf).context("reading a chunk of flood output")?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn force_attach() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 = daemon_proc.attach("sh1", Default::default()).context("attaching from tty1")?;
    let mut line_matcher1 = tty1.line_matcher()?;
    tty1.run_cmd("export MYVAR='set_from_tty1'")?;
    tty1.run_cmd("echo $MYVAR")?;
    // read some output to make sure the var is set by the time
    // we force-attach
    line_matcher1.scan_until_re("set_from_tty1$")?;

    let mut tty2 = daemon_proc
        .attach("sh1", AttachArgs { force: true, ..Default::default() })
        .context("attaching from tty2")?;
    let mut line_matcher2 = tty2.line_matcher()?;
    tty2.run_cmd("echo $MYVAR")?;
    line_matcher2.match_re("set_from_tty1$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn busy() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 = daemon_proc.attach("sh1", Default::default()).context("attaching from tty1")?;
    let mut line_matcher1 = tty1.line_matcher()?;
    tty1.run_cmd("echo foo")?; // make sure the shell is up and running
    line_matcher1.scan_until_re("foo$")?;

    let mut tty2 = daemon_proc.attach("sh1", Default::default()).context("attaching from tty2")?;
    let mut line_matcher2 = tty2.stderr_line_matcher()?;
    line_matcher2.scan_until_re("already has a terminal attached$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn blank_session_not_allowed() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 = daemon_proc.attach("", Default::default()).context("attaching from tty1")?;
    let mut line_matcher1 = tty1.stderr_line_matcher()?;
    line_matcher1.scan_until_re("blank session names are not allowed")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn whitespace_session_not_allowed() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;

    let mut tty1 =
        daemon_proc.attach("this\tbad", Default::default()).context("attaching from tty1")?;
    let mut line_matcher1 = tty1.stderr_line_matcher()?;
    line_matcher1.scan_until_re("whitespace is not allowed in session names")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn daemon_hangup() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    // make sure the shell is up and running
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo foo")?;
    line_matcher.scan_until_re("foo$")?;

    daemon_proc.proc_kill()?;

    let exit_status = attach_proc.proc.wait()?;
    assert!(!exit_status.success());

    Ok(())
}

#[test]
#[timeout(30000)]
fn default_keybinding_detach() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    a1.run_cmd("export MYVAR=someval")?;
    a1.run_cmd("echo $MYVAR")?;
    lm1.scan_until_re("someval$")?;

    a1.run_raw_cmd(vec![0, 17])?; // Ctrl-Space Ctrl-q
    let exit_status = a1.proc.wait()?;
    dbg!(exit_status);
    assert!(exit_status.success());

    waiter.wait_event("daemon-bidi-stream-done")?;

    let mut a2 =
        daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
    let mut lm2 = a2.line_matcher()?;

    a2.run_cmd("echo $MYVAR")?;
    lm2.scan_until_re("someval$")?;

    Ok(())
}

// test to exercise the code path where a keybinding
// shows up in two different input chunks
#[test]
#[timeout(30000)]
fn keybinding_input_shear() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    a1.run_cmd("export MYVAR=someval")?;
    a1.run_cmd("echo $MYVAR")?;
    lm1.scan_until_re("someval$")?;

    a1.run_raw(vec![0])?; // Ctrl-Space
    thread::sleep(time::Duration::from_millis(100));
    a1.run_raw(vec![17])?; // Ctrl-q
    a1.proc.wait()?;

    waiter.wait_event("daemon-bidi-stream-done")?;

    let mut a2 =
        daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
    let mut lm2 = a2.line_matcher()?;

    a2.run_cmd("echo $MYVAR")?;
    lm2.scan_until_re("someval$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn keybinding_strip_keys() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "long_noop_keybinding.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    // the keybinding is 5 'a' chars in a row, so they should get stripped out
    a1.run_cmd("echo baaaaad")?;
    lm1.scan_until_re("bd$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn keybinding_strip_keys_split() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "long_noop_keybinding.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    // the keybinding is 5 'a' chars in a row, so they should get stripped out
    a1.run_raw("echo ba".bytes().collect())?;
    thread::sleep(time::Duration::from_millis(50));
    a1.run_raw("aa".bytes().collect())?;
    thread::sleep(time::Duration::from_millis(50));
    a1.run_raw("aad\n".bytes().collect())?;
    lm1.scan_until_re("bd")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn keybinding_partial_match_nostrip() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "long_noop_keybinding.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    // the keybinding is 5 'a' chars in a row, this has only 3
    a1.run_cmd("echo baaad")?;
    lm1.scan_until_re("baaad$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn keybinding_partial_match_nostrip_split() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "long_noop_keybinding.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    // the keybinding is 5 'a' chars in a row, this has only 3
    a1.run_raw("echo ba".bytes().collect())?;
    thread::sleep(time::Duration::from_millis(50));
    a1.run_raw("a".bytes().collect())?;
    thread::sleep(time::Duration::from_millis(50));
    a1.run_raw("ad\n".bytes().collect())?;
    lm1.scan_until_re("baaad$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn custom_keybinding_detach() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("custom_detach_keybinding.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    let mut a1 = daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
    let mut lm1 = a1.line_matcher()?;

    a1.run_cmd("export MYVAR=someval")?;
    a1.run_cmd("echo $MYVAR")?;
    lm1.scan_until_re("someval$")?;

    a1.run_raw_cmd(vec![22, 23, 7])?; // Ctrl-v Ctrl-w Ctrl-g
    a1.proc.wait()?;

    waiter.wait_event("daemon-bidi-stream-done")?;

    let mut a2 =
        daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
    let mut lm2 = a2.line_matcher()?;

    a2.run_cmd("echo $MYVAR")?;
    lm2.match_re("someval$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn injects_term_even_with_env_config() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("user_env.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs {
                extra_env: vec![(String::from("TERM"), String::from("dumb"))],
                ..Default::default()
            },
        )
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    waiter.wait_event("daemon-wrote-s2c-chunk")?; // resize prompt redraw
    attach_proc.run_cmd("echo $SOME_CUSTOM_ENV_VAR")?;
    line_matcher.scan_until_re("customvalue$")?;

    attach_proc.run_cmd("echo $TERM")?;
    line_matcher.match_re("dumb$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn injects_local_env_vars() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs {
                extra_env: vec![
                    (String::from("DISPLAY"), String::from(":0")),
                    (String::from("LANG"), String::from("fakelang")),
                ],
                ..Default::default()
            },
        )
        .context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("echo $DISPLAY")?;
    line_matcher.scan_until_re(":0$")?;

    attach_proc.run_cmd("echo $LANG")?;
    line_matcher.scan_until_re("fakelang$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn has_right_default_path() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("no_etc_environment.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("echo $PATH")?;
    line_matcher.scan_until_re("/usr/bin:/bin:/usr/sbin:/sbin$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn screen_restore() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("restore_screen.toml", DaemonArgs::default())
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
        line_matcher.never_matches("^.*SHPOOL_PROMPT_SETUP_SENTINEL.*$")?;

        // the re-attach should redraw the screen for us, so we should
        // get a line with "foo" as part of the re-drawn screen.
        line_matcher.scan_until_re("foo$")?;

        attach_proc.proc.kill()?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn screen_wide_restore() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("restore_screen.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy")?;
        line_matcher.scan_until_re("ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy$")?;
    }

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        // the re-attach should redraw the screen for us, so we should
        // get a line with the full echo result as part of the re-drawn screen.
        line_matcher.scan_until_re("ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy$")?;
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn lines_restore() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("restore_lines.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo foo")?;
        attach_proc.run_cmd("echo")?;
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

// Test to make sure that when we do a restore, we don't send back too many
// bytes in once chunk. The attach client has a fixed size buffer it reads into,
// and it will crash if it gets sent a chunk with too large a length.
#[test]
#[timeout(30000)]
fn lines_big_chunk_restore() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("restore_many_lines.toml", DaemonArgs::default())
            .context("starting daemon proc")?;
    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-wrote-s2c-chunk", "daemon-bidi-stream-done"]);

    // BUF_SIZE from src/consts.rs
    let max_chunk_size = 1024 * 16;

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        // wait for shell output to avoid racing against the shell
        waiter.wait_event("daemon-wrote-s2c-chunk")?;

        // generate a bunch of data that will cause the restore buffer to be too large
        // for a single chunk
        let blob = format!("echo {}", (0..max_chunk_size).map(|_| "x").collect::<String>());
        attach_proc.run_cmd(blob.as_str())?;
        line_matcher.scan_until_re("xx$")?;

        attach_proc.run_cmd("echo food")?;
        line_matcher.match_re("food$")?;
    }

    // wait until the daemon has noticed that the connection
    // has dropped before we attempt to open the connection again
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    {
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut reader = std::io::BufReader::new(
            attach_proc.proc.stdout.take().ok_or(anyhow!("missing stdout"))?,
        );

        let mut output = vec![];
        reader.read_until(b'd', &mut output)?;
        let chunk = String::from_utf8_lossy(&output[..]);
        assert!(chunk.contains("foo"));
    }

    Ok(())
}

#[test]
#[timeout(30000)]
fn exits_with_same_status_as_shell() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh", Default::default()).context("starting attach proc")?;

    attach_proc.run_cmd("exit 19")?;

    assert_eq!(
        attach_proc
            .proc
            .wait()
            .context("waiting for attach proc to exit")?
            .code()
            .ok_or(anyhow!("no exit code"))?,
        19
    );

    Ok(())
}

#[test]
#[timeout(30000)]
fn ttl_hangup() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        .attach("sh1", AttachArgs { ttl: Some(time::Duration::from_secs(1)), ..Default::default() })
        .context("starting attach proc")?;

    // ensure the shell is up and running
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo hi")?;
    line_matcher.scan_until_re("hi$")?;

    // sleep long enough for the reaper to clobber the thread
    thread::sleep(time::Duration::from_millis(1200));

    let listout = daemon_proc.list()?;
    assert!(!String::from_utf8_lossy(listout.stdout.as_slice()).contains("sh1"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn ttl_no_hangup_yet() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs { ttl: Some(time::Duration::from_secs(1000)), ..Default::default() },
        )
        .context("starting attach proc")?;

    // ensure the shell is up and running
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo hi")?;
    line_matcher.scan_until_re("hi$")?;

    let listout = daemon_proc.list()?;
    assert!(String::from_utf8_lossy(listout.stdout.as_slice()).contains("sh1"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn prompt_prefix_bash() -> anyhow::Result<()> {
    let daemon_proc = support::daemon::Proc::new("prompt_prefix_bash.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    // we have to manually spawn the child proc rather than using the support
    // util because the line matcher gets bound only after the process gets
    // spawned, so there will be a race between the prompt printing and
    // binding the line matcher if we use the wrapper util.
    let mut child = Command::new(support::shpool_bin()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket")
        .arg(&daemon_proc.socket_path)
        .arg("--config-file")
        .arg(support::testdata_file("prompt_prefix_bash.toml"))
        .arg("attach")
        .arg("sh1")
        .spawn()
        .context("spawning attach process")?;

    // The attach shell should be spawned and have read the
    // initial prompt after half a second.
    std::thread::sleep(time::Duration::from_millis(500));
    child.kill().context("killing child")?;

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    assert!(stderr_str.is_empty());

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;
    eprintln!("stdout_str: {}", stdout_str);
    let stdout_re = Regex::new(".*session_name=sh1 prompt>.*")?;
    assert!(stdout_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // hard-coded /usr/bin/zsh path
fn prompt_prefix_zsh() -> anyhow::Result<()> {
    let daemon_proc = support::daemon::Proc::new("prompt_prefix_zsh.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    // see the bash case for why
    let mut child = Command::new(support::shpool_bin()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket")
        .arg(&daemon_proc.socket_path)
        .arg("--config-file")
        .arg(support::testdata_file("prompt_prefix_zsh.toml"))
        .arg("attach")
        .arg("sh1")
        .spawn()
        .context("spawning attach process")?;

    // The attach shell should be spawned and have read the
    // initial prompt after half a second.
    std::thread::sleep(time::Duration::from_millis(500));
    child.kill().context("killing child")?;

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    assert!(stderr_str.is_empty());

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;
    let stdout_re = Regex::new(".*session_name=sh1.*")?;
    assert!(stdout_re.is_match(&stdout_str));

    Ok(())
}

// This has stopped working in CI. Probably due to a fish version
// change or something.
#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // hard-coded /usr/bin/fish path
fn prompt_prefix_fish() -> anyhow::Result<()> {
    let daemon_proc = support::daemon::Proc::new("prompt_prefix_fish.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    // see the bash case for why
    let mut child = Command::new(support::shpool_bin()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket")
        .arg(&daemon_proc.socket_path)
        .arg("--config-file")
        .arg(support::testdata_file("prompt_prefix_fish.toml"))
        .arg("attach")
        .arg("sh1")
        .spawn()
        .context("spawning attach process")?;

    // The attach shell should be spawned and have read the
    // initial prompt after half a second.
    std::thread::sleep(time::Duration::from_millis(500));
    child.kill().context("killing child")?;

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    assert!(stderr_str.is_empty());

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;
    let stdout_re = Regex::new(".*session_name=sh1.*")?;
    assert!(stdout_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
fn motd_dump() -> anyhow::Result<()> {
    // set up the config
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let motd_file = tmp_dir.path().join("motd.txt");
    {
        let mut f = fs::File::create(&motd_file)?;
        f.write_all("MOTD_MSG".as_bytes())?;
    }
    let config_tmpl = fs::read_to_string(support::testdata_file("motd_dump.toml.tmpl"))?;
    let config_contents = config_tmpl.replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap());
    let config_file = tmp_dir.path().join("motd_dump.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    // spawn a daemon based on our custom config
    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;
    line_matcher.scan_until_re(".*MOTD_MSG.*")?;

    Ok(())
}

/// Attach to the given daemon and wait for the given duration, then
/// capture all the output the attach process saw. This allows for sniffing
/// for the presence of motd messages.
fn snapshot_attach_output<P: AsRef<OsStr>>(
    daemon: &support::daemon::Proc,
    config_file: P,
    quiescence_dur: time::Duration,
    session_name: &str,
) -> anyhow::Result<String> {
    let mut child = Command::new(support::shpool_bin()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket")
        .arg(&daemon.socket_path)
        .arg("--config-file")
        .arg(config_file)
        .arg("attach")
        .arg(session_name)
        .spawn()
        .context("spawning attach process")?;

    // wait for things to settle, then kill
    std::thread::sleep(quiescence_dur);
    child.kill().context("killing child")?;

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    if !stderr_str.is_empty() {
        return Err(anyhow!("expected no stderr, got '{}'", stderr_str));
    }

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;
    Ok(stdout_str)
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // pager pty output issue
fn motd_pager() -> anyhow::Result<()> {
    // set up the config
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let motd_file = tmp_dir.path().join("motd.txt");
    {
        let mut f = fs::File::create(&motd_file)?;
        f.write_all("MOTD_MSG\n".as_bytes())?;
    }
    let config_file = tmp_dir.path().join("motd_pager.toml");

    // spawn a daemon based on our custom config
    let mut daemon_proc = support::daemon::Proc::new(
        &config_file,
        DaemonArgs {
            extra_env: vec![(String::from("TERM"), String::from("xterm"))],
            ..DaemonArgs::default()
        },
    )
    .context("starting daemon proc")?;

    // Update the config and wait for it to get picked up.
    // Doing this after the daemon starts tests to make sure
    // that we can handle dynamic config changes to the
    // motd settings.
    let config_tmpl = fs::read_to_string(support::testdata_file("motd_pager.toml.tmpl"))?;
    let config_contents = config_tmpl.replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap());
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }
    daemon_proc.await_event("daemon-reload-config")?;

    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh1",
    )?;
    // Scan for a fragment of less output.
    let stdout_file_re = Regex::new(".*\\(END\\).*")?;
    assert!(stdout_file_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // pager pty output issue
fn motd_debounced_pager_debounces() -> anyhow::Result<()> {
    // set up the config
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let motd_file = tmp_dir.path().join("motd.txt");
    {
        let mut f = fs::File::create(&motd_file)?;
        f.write_all("MOTD_MSG\n".as_bytes())?;
    }
    let config_tmpl =
        fs::read_to_string(support::testdata_file("motd_pager_1d_debounce.toml.tmpl"))?;
    let config_contents = config_tmpl.replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap());
    let config_file = tmp_dir.path().join("motd_pager.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    // spawn a daemon based on our custom config
    let daemon_proc = support::daemon::Proc::new(
        &config_file,
        DaemonArgs {
            extra_env: vec![(String::from("TERM"), String::from("xterm"))],
            ..DaemonArgs::default()
        },
    )
    .context("starting daemon proc")?;

    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh1",
    )?;
    // scan for a fragment of less output
    let stdout_file_re = Regex::new(".*\\(END\\).*")?;
    assert!(stdout_file_re.is_match(&stdout_str));

    // We should see not the message again when we try again immediately.
    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh2",
    )?;
    assert!(!stdout_file_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // pager pty output issue
fn motd_debounced_pager_unbounces() -> anyhow::Result<()> {
    // set up the config
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let motd_file = tmp_dir.path().join("motd.txt");
    {
        let mut f = fs::File::create(&motd_file)?;
        f.write_all("MOTD_MSG\n".as_bytes())?;
    }
    let config_tmpl =
        fs::read_to_string(support::testdata_file("motd_pager_1s_debounce.toml.tmpl"))?;
    let config_contents = config_tmpl.replace("TMP_MOTD_MSG_FILE", motd_file.to_str().unwrap());
    let config_file = tmp_dir.path().join("motd_pager.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    // spawn a daemon based on our custom config
    let daemon_proc = support::daemon::Proc::new(
        &config_file,
        DaemonArgs {
            extra_env: vec![(String::from("TERM"), String::from("xterm"))],
            ..DaemonArgs::default()
        },
    )
    .context("starting daemon proc")?;

    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh1",
    )?;
    // scan for a fragment of less output
    let stdout_file_re = Regex::new(".*\\(END\\).*")?;
    assert!(stdout_file_re.is_match(&stdout_str));

    // sleep for 1.1 seconds because the debounce duration is 1 second.
    std::thread::sleep(time::Duration::from_millis(1100));

    // We should see the message again when we try again after debounce.
    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh2",
    )?;
    assert!(stdout_file_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)] // pager pty output issue
fn motd_env_test_pager_preserves_term_env_var() -> anyhow::Result<()> {
    // set up the config
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let config_tmpl = fs::read_to_string(support::testdata_file("motd_pager_env_test.toml.tmpl"))?;
    let config_contents = config_tmpl.replace(
        "MOTD_ENV_TEST_SCRIPT",
        support::testdata_file("motd_env_test_script.sh").to_str().unwrap(),
    );
    let config_file = tmp_dir.path().join("motd_pager.toml");
    {
        let mut f = fs::File::create(&config_file)?;
        f.write_all(config_contents.as_bytes())?;
    }

    // spawn a daemon based on our custom config
    let daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    let stdout_str = snapshot_attach_output(
        &daemon_proc,
        &config_file,
        time::Duration::from_millis(500),
        "sh1",
    )?;
    // Scan for a fragment of less output.
    eprintln!("STDOUT: {stdout_str}");
    let stdout_file_re = Regex::new(".*TERM=testval.*")?;
    assert!(stdout_file_re.is_match(&stdout_str));

    Ok(())
}

#[test]
#[timeout(30000)]
fn dynamic_config_change() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("building config in {:?}", tmp_dir.path());
    let config_tmpl = fs::read_to_string(support::testdata_file("dynamic_config.toml.tmpl"))?;
    let config_file = tmp_dir.path().join("motd_pager.toml");
    fs::write(&config_file, &config_tmpl)?;

    let mut daemon_proc =
        support::daemon::Proc::new(&config_file, DaemonArgs { ..DaemonArgs::default() })
            .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo $CHANGING_VAR")?;
    line_matcher.scan_until_re("REPLACE_ME$")?;

    // Create waiter right before changing config file, since reload can also happen
    // right after daemon startup.
    let mut waiter = daemon_proc
        .events
        .take()
        .unwrap()
        .waiter(["daemon-config-watcher-file-change", "daemon-reload-config"]);

    // Change the config contents on the fly
    let config_contents = config_tmpl.replace("REPLACE_ME", "NEW_VALUE");
    fs::write(&config_file, config_contents)?;

    // Wait for reload to happen since there is debounce time.
    waiter.wait_event("daemon-config-watcher-file-change")?;
    waiter.wait_event("daemon-reload-config")?;

    // When we spawn a new session, it should pick up the new value
    let mut attach_proc =
        daemon_proc.attach("sh2", Default::default()).context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;
    attach_proc.run_cmd("echo $CHANGING_VAR")?;
    line_matcher.scan_until_re("NEW_VALUE$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn fresh_shell_does_not_have_prompt_setup_code() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;

    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut reader =
        std::io::BufReader::new(attach_proc.proc.stdout.take().ok_or(anyhow!("missing stdout"))?);

    let mut output = vec![];
    reader.read_until(b'>', &mut output)?;
    let chunk = String::from_utf8_lossy(&output[..]);
    assert!(!chunk.contains("SHPOOL__OLD_PROMPT_COMMAND"));

    Ok(())
}

#[test]
#[timeout(30000)]
fn autodaemonize() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    eprintln!("testing autodaemonization in {:?}", tmp_dir.path());

    let mut socket_path = PathBuf::from(tmp_dir.path());
    socket_path.push("control.sock");

    let mut log_file = PathBuf::from(tmp_dir.path());
    log_file.push("attach.log");

    // we have to manually spawn the child because the whole point is that there
    // isn't a daemon yet so we can't use the attach method.
    let mut child = Command::new(support::shpool_bin()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--daemonize")
        .arg("--socket")
        .arg(socket_path)
        .arg("--log-file")
        .arg(log_file)
        .arg("--config-file")
        .arg(support::testdata_file("norc.toml"))
        .arg("attach")
        .arg("sh1")
        .spawn()
        .context("spawning attach process")?;

    // After half a second, the daemon should have spanwed
    std::thread::sleep(time::Duration::from_millis(500));
    child.kill().context("killing child")?;

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;
    let stdout_re = Regex::new(".*prompt>.*")?;
    assert!(stdout_re.is_match(&stdout_str));

    // best effort attempt to clean up after ourselves
    Command::new("pkill")
        .arg("-f")
        .arg("shpool-test-autodaemonize")
        .output()
        .context("running cleanup process")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn config_tmp_default_dir() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("tmp_default_dir.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        .attach(
            "sh1",
            AttachArgs { config: Some(String::from("tmp_default_dir.toml")), ..Default::default() },
        )
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("pwd")?;
    line_matcher.scan_until_re("tmp$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn cli_tmp_dir() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let mut attach_proc = daemon_proc
        .attach("sh1", AttachArgs { dir: Some(String::from("/tmp")), ..Default::default() })
        .context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    attach_proc.run_cmd("pwd")?;
    line_matcher.scan_until_re("tmp$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn version_mismatch_client_newer() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs {
            extra_env: vec![(String::from("SHPOOL_TEST__OVERRIDE_VERSION"), String::from("0.0.0"))],
            ..DaemonArgs::default()
        },
    )
    .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
    let mut line_matcher = attach_proc.line_matcher()?;
    let mut stderr_line_matcher = attach_proc.stderr_line_matcher()?;

    // we should see a warning prompting us
    stderr_line_matcher.scan_until_re("is newer.*try restarting your daemon$")?;
    stderr_line_matcher.scan_until_re("hit enter to continue.*$")?;
    attach_proc.run_cmd("")?; // continue through it

    // not really needed, just here to test the events system
    attach_proc.run_cmd("echo hi")?;
    line_matcher.scan_until_re("hi$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
fn detaches_on_null_stdin() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new("norc.toml", DaemonArgs::default())
        .context("starting daemon proc")?;
    let _attach_proc = daemon_proc
        .attach("sh1", AttachArgs { null_stdin: true, ..Default::default() })
        .context("starting attach proc")?;

    // not really needed, just here to test the events system
    daemon_proc.await_event("daemon-about-to-listen")?;

    loop {
        thread::sleep(time::Duration::from_millis(300));

        let list_json_out = daemon_proc.list_json()?;
        let list_str = String::from_utf8_lossy(&list_json_out.stdout);
        eprintln!("LIST OUTPUT: '{}'", list_str);

        let list_blob: serde_json::Value = serde_json::from_str(&list_str)?;

        if list_blob["sessions"].as_array().unwrap().is_empty() {
            continue;
        }
        assert_eq!(list_blob["sessions"][0]["name"].as_str().unwrap(), "sh1");
        if list_blob["sessions"][0]["status"].as_str().unwrap() != "Disconnected" {
            continue;
        }

        return Ok(());
    }
}

#[ignore] // TODO: re-enable, this test if flaky
#[test]
fn up_arrow_no_crash() -> anyhow::Result<()> {
    let mut daemon_proc = support::daemon::Proc::new(
        "norc.toml",
        DaemonArgs { listen_events: false, ..DaemonArgs::default() },
    )
    .context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // Before we put the pty into raw mode, this would
    // cause crashes.
    attach_proc.run_raw_cmd(vec![27, 91, 65, 10])?; // up arrow
    line_matcher.match_re("logout$")?;

    Ok(())
}

#[test]
#[timeout(30000)]
#[cfg_attr(target_os = "macos", ignore)]
fn dynamic_session_restore_mode() -> anyhow::Result<()> {
    let tmp_dir = tmpdir::Dir::new("/tmp/shpool-test")?;
    let config_tmpl = fs::read_to_string(support::testdata_file("dynamic_restore.toml.tmpl"))?;
    let config_file = tmp_dir.path().join("shpool.toml");

    // Start with "simple" mode
    let config_contents = config_tmpl.replace("REPLACE_ME", "\"simple\"");
    fs::write(&config_file, &config_contents)?;

    let mut daemon_proc = support::daemon::Proc::new(&config_file, DaemonArgs::default())
        .context("starting daemon proc")?;

    // Register all expected events up front.
    let mut waiter = daemon_proc.events.take().unwrap().waiter([
        "daemon-bidi-stream-done", // s1 detach
        "daemon-reload-config",    // config reload
        "daemon-bidi-stream-done", // s2 detach
    ]);

    // Create session s1
    {
        let mut a1 = daemon_proc.attach("s1", Default::default()).context("starting s1")?;
        let mut lm1 = a1.line_matcher()?;
        a1.run_cmd("echo foo")?;
        lm1.scan_until_re("foo$")?;
    } // detach s1
    waiter.wait_event("daemon-bidi-stream-done")?;

    // Change config to { lines = 2 }
    let config_contents = config_tmpl.replace("REPLACE_ME", "{ lines = 2 }");
    fs::write(&config_file, config_contents)?;

    waiter.wait_event("daemon-reload-config")?;

    // Create session s2
    {
        let mut a2 = daemon_proc.attach("s2", Default::default()).context("starting s2")?;
        let mut lm2 = a2.line_matcher()?;
        a2.run_cmd("echo bar")?;
        lm2.scan_until_re("bar$")?;
    } // detach s2
    daemon_proc.events = Some(waiter.wait_final_event("daemon-bidi-stream-done")?);

    // Verify s1 still has "simple" behavior (no restore)
    {
        let mut a1 = daemon_proc.attach("s1", Default::default()).context("reattaching s1")?;
        let mut lm1 = a1.line_matcher()?;
        lm1.never_matches("foo$")?;

        a1.run_cmd("echo s1_alive")?;
        lm1.scan_until_re("s1_alive$")?;

        a1.proc.kill()?;
    }

    // Verify s2 has "lines = 2" behavior (restores bar)
    {
        let mut a2 = daemon_proc.attach("s2", Default::default()).context("reattaching s2")?;
        let mut lm2 = a2.line_matcher()?;
        // It SHOULD see "bar" again on re-attach
        lm2.scan_until_re("bar$")?;
    }

    Ok(())
}
