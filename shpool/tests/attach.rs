use std::{
    env, fs,
    io::BufRead,
    io::Read,
    process::{Command, Stdio},
    thread, time,
};

use anyhow::{anyhow, Context};
use ntest::timeout;

mod support;

use crate::support::daemon::AttachArgs;

#[test]
#[timeout(30000)]
fn happy_path() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

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
    })
}

#[test]
#[timeout(30000)]
fn symlink_ssh_auth_sock() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

        let fake_auth_sock_tgt = daemon_proc.tmp_dir.join("ssh-auth-sock-target.fake");
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
        line_matcher.match_re(r#".*sh1/ssh-auth-sock.socket ->.*ssh-auth-sock-target.fake$"#)?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn missing_ssh_auth_sock() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

        let fake_auth_sock_tgt = daemon_proc.tmp_dir.join("ssh-auth-sock-target.fake");
        fs::File::create(&fake_auth_sock_tgt)?;

        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        waiter.wait_event("daemon-wrote-s2c-chunk")?; // resize prompt re-draw
        attach_proc.run_cmd("ls -l $SSH_AUTH_SOCK")?;
        line_matcher.match_re(r#".*No such file or directory$"#)?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn fresh_shell_draws_prompt() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
        let mut reader = std::io::BufReader::new(
            attach_proc.proc.stdout.take().ok_or(anyhow!("missing stdout"))?,
        );

        let mut output = vec![];
        reader.read_until(b'>', &mut output)?;
        let chunk = String::from_utf8_lossy(&output[..]);
        assert!(chunk.contains("prompt"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn config_disable_symlink_ssh_auth_sock() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("disable_symlink_ssh_auth_sock.toml", true)
                .context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-wrote-s2c-chunk"]);

        let fake_auth_sock_tgt = daemon_proc.tmp_dir.join("ssh-auth-sock-target.fake");
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
        line_matcher.match_re(r#".*No such file or directory$"#)?;

        Ok(())
    })
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
#[timeout(30000)]
fn bounce() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

            let mut line_matcher = attach_proc.line_matcher()?;

            attach_proc.run_cmd("export MYVAR=1")?;
            attach_proc.run_cmd("echo $MYVAR")?;
            line_matcher.match_re("1$")?;
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
    })
}

#[test]
#[timeout(30000)]
fn two_at_once() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;

        let mut attach_proc1 =
            daemon_proc.attach("sh1", Default::default()).context("starting sh1")?;
        let mut attach_proc2 =
            daemon_proc.attach("sh2", Default::default()).context("starting sh2")?;

        let mut line_matcher1 = attach_proc1.line_matcher()?;
        let mut line_matcher2 = attach_proc2.line_matcher()?;

        attach_proc1.run_cmd("echo proc1").context("proc1 echo")?;
        line_matcher1.match_re("proc1$").context("proc1 match")?;

        attach_proc2.run_cmd("echo proc2").context("proc2 echo")?;
        line_matcher2.match_re("proc2$").context("proc2 match")?;

        Ok(())
    })
}

// test the attach process getting killed, then re-attaching to the
// same shell session.
#[test]
#[timeout(30000)]
fn explicit_exit() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);
        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

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
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("reattaching")?;

            let mut line_matcher = attach_proc.line_matcher()?;

            attach_proc.run_cmd("echo ${MYVAR:-second}")?;
            line_matcher.match_re("second$")?;
        }

        Ok(())
    })
}

// Test the attach process getting killed, then re-attaching to the
// same shell session.
#[ignore] // this test is flaky in ci. TODO: re-enable
#[test]
#[timeout(30000)]
fn exit_immediate_drop() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;

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
    })
}

#[test]
#[timeout(30000)]
fn output_flood() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;
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
    })
}

#[test]
#[timeout(30000)]
fn force_attach() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;

        let mut tty1 =
            daemon_proc.attach("sh1", Default::default()).context("attaching from tty1")?;
        let mut line_matcher1 = tty1.line_matcher()?;
        tty1.run_cmd("export MYVAR='set_from_tty1'")?;
        tty1.run_cmd("echo $MYVAR")?;
        // read some output to make sure the var is set by the time
        // we force-attach
        line_matcher1.match_re("set_from_tty1$")?;

        let mut tty2 = daemon_proc
            .attach("sh1", AttachArgs { force: true, ..Default::default() })
            .context("attaching from tty2")?;
        let mut line_matcher2 = tty2.line_matcher()?;
        tty2.run_cmd("echo $MYVAR")?;
        line_matcher2.match_re("set_from_tty1$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn busy() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;

        let mut tty1 =
            daemon_proc.attach("sh1", Default::default()).context("attaching from tty1")?;
        let mut line_matcher1 = tty1.line_matcher()?;
        tty1.run_cmd("echo foo")?; // make sure the shell is up and running
        line_matcher1.match_re("foo$")?;

        let mut tty2 =
            daemon_proc.attach("sh1", Default::default()).context("attaching from tty2")?;
        let mut line_matcher2 = tty2.stderr_line_matcher()?;
        line_matcher2.match_re("already has a terminal attached$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn daemon_hangup() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        // make sure the shell is up and running
        let mut line_matcher = attach_proc.line_matcher()?;
        attach_proc.run_cmd("echo foo")?;
        line_matcher.match_re("foo$")?;

        daemon_proc.proc.kill()?;

        let exit_status = attach_proc.proc.wait()?;
        assert!(!exit_status.success());

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn default_keybinding_detach() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        a1.run_cmd("export MYVAR=someval")?;
        a1.run_cmd("echo $MYVAR")?;
        lm1.match_re("someval$")?;

        a1.run_raw_cmd(vec![0, 17])?; // Ctrl-Space Ctrl-q
        a1.proc.wait()?;

        waiter.wait_event("daemon-bidi-stream-done")?;

        let mut a2 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
        let mut lm2 = a2.line_matcher()?;

        a2.run_cmd("echo $MYVAR")?;
        lm2.match_re("someval$")?;

        Ok(())
    })
}

// test to exercise the code path where a keybinding
// shows up in two different input chunks
#[test]
#[timeout(30000)]
fn keybinding_input_shear() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        a1.run_cmd("export MYVAR=someval")?;
        a1.run_cmd("echo $MYVAR")?;
        lm1.match_re("someval$")?;

        a1.run_raw(vec![0])?; // Ctrl-Space
        thread::sleep(time::Duration::from_millis(100));
        a1.run_raw(vec![17])?; // Ctrl-q
        a1.proc.wait()?;

        waiter.wait_event("daemon-bidi-stream-done")?;

        let mut a2 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
        let mut lm2 = a2.line_matcher()?;

        a2.run_cmd("echo $MYVAR")?;
        lm2.match_re("someval$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn keybinding_strip_keys() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("long_noop_keybinding.toml", false)
            .context("starting daemon proc")?;
        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        // the keybinding is 5 'a' chars in a row, so they should get stripped out
        a1.run_cmd("echo baaaaad")?;
        lm1.match_re("bd$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn keybinding_strip_keys_split() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("long_noop_keybinding.toml", false)
            .context("starting daemon proc")?;
        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        // the keybinding is 5 'a' chars in a row, so they should get stripped out
        a1.run_raw("echo ba".bytes().collect())?;
        thread::sleep(time::Duration::from_millis(50));
        a1.run_raw("aa".bytes().collect())?;
        thread::sleep(time::Duration::from_millis(50));
        a1.run_raw("aad\n".bytes().collect())?;
        lm1.match_re("bd$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn keybinding_partial_match_nostrip() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("long_noop_keybinding.toml", false)
            .context("starting daemon proc")?;
        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        // the keybinding is 5 'a' chars in a row, this has only 3
        a1.run_cmd("echo baaad")?;
        lm1.match_re("baaad$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn keybinding_partial_match_nostrip_split() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("long_noop_keybinding.toml", false)
            .context("starting daemon proc")?;
        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        // the keybinding is 5 'a' chars in a row, this has only 3
        a1.run_raw("echo ba".bytes().collect())?;
        thread::sleep(time::Duration::from_millis(50));
        a1.run_raw("a".bytes().collect())?;
        thread::sleep(time::Duration::from_millis(50));
        a1.run_raw("ad\n".bytes().collect())?;
        lm1.match_re("baaad$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn custom_keybinding_detach() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("custom_detach_keybinding.toml", true)
            .context("starting daemon proc")?;
        let mut waiter = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        let mut a1 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc")?;
        let mut lm1 = a1.line_matcher()?;

        a1.run_cmd("export MYVAR=someval")?;
        a1.run_cmd("echo $MYVAR")?;
        lm1.match_re("someval$")?;

        a1.run_raw_cmd(vec![22, 23, 7])?; // Ctrl-v Ctrl-w Ctrl-g
        a1.proc.wait()?;

        waiter.wait_event("daemon-bidi-stream-done")?;

        let mut a2 =
            daemon_proc.attach("sess", Default::default()).context("starting attach proc 2")?;
        let mut lm2 = a2.line_matcher()?;

        a2.run_cmd("echo $MYVAR")?;
        lm2.match_re("someval$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn injects_term_even_with_env_config() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("user_env.toml", true).context("starting daemon proc")?;
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
        line_matcher.match_re("customvalue$")?;

        attach_proc.run_cmd("echo $TERM")?;
        line_matcher.match_re("dumb$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn injects_display() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut attach_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs {
                    extra_env: vec![(String::from("DISPLAY"), String::from(":0"))],
                    ..Default::default()
                },
            )
            .context("starting attach proc")?;
        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $DISPLAY")?;
        line_matcher.match_re(":0$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn has_right_default_path() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut attach_proc =
            daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

        let mut line_matcher = attach_proc.line_matcher()?;

        attach_proc.run_cmd("echo $PATH")?;
        line_matcher.match_re("/usr/bin:/bin:/usr/sbin:/sbin$")?;

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn mismatch_exe() -> anyhow::Result<()> {
    support::dump_err(|| {
        let daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut copied_exe_path = daemon_proc.tmp_dir.clone();
        copied_exe_path.push("copied-shpool");
        fs::copy(support::shpool_bin()?, &copied_exe_path)?;

        let log_file = daemon_proc.tmp_dir.join("attach.log");
        eprintln!("spawning attach proc with log {:?}", &log_file);

        let mut child = Command::new(copied_exe_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .arg("-v")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&daemon_proc.socket_path)
            .env_clear()
            .env("XDG_RUNTIME_DIR", env::var("XDG_RUNTIME_DIR")?)
            .arg("attach")
            .arg("sh1")
            .spawn()
            .context("spawning attach proc")?;

        std::thread::sleep(time::Duration::from_millis(500));
        child.kill().context("killing child")?;

        let mut stderr = child.stderr.take().context("missing stderr")?;
        let mut stderr_str = String::from("");
        stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
        assert!(stderr_str.contains("shpool: warn: attach binary differs from daemon binary"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn screen_restore() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("restore_screen.toml", true)
            .context("starting daemon proc")?;
        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
            let mut line_matcher = attach_proc.line_matcher()?;

            attach_proc.run_cmd("echo foo")?;
            line_matcher.match_re("foo$")?;
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
            line_matcher.match_re("foo$")?;
        }

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn screen_wide_restore() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("restore_screen.toml", true)
            .context("starting daemon proc")?;
        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
            let mut line_matcher = attach_proc.line_matcher()?;

            attach_proc.run_cmd("echo ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy")?;
            line_matcher.match_re("ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy$")?;
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
            line_matcher.match_re("ooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooyooooxooooy$")?;
        }

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn lines_restore() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("restore_lines.toml", true)
            .context("starting daemon proc")?;
        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
            let mut line_matcher = attach_proc.line_matcher()?;

            attach_proc.run_cmd("echo foo")?;
            line_matcher.match_re("foo$")?;
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
            line_matcher.match_re("foo$")?;
        }

        Ok(())
    })
}

// Test to make sure that when we do a restore, we don't send back too many
// bytes in once chunk. The attach client has a fixed size buffer it reads into,
// and it will crash if it gets sent a chunk with too large a length.
#[test]
#[timeout(30000)]
fn lines_big_chunk_restore() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new("restore_many_lines.toml", true)
            .context("starting daemon proc")?;
        let bidi_done_w = daemon_proc.events.take().unwrap().waiter(["daemon-bidi-stream-done"]);

        // BUF_SIZE from src/consts.rs
        let max_chunk_size = 1024 * 16;

        {
            let mut attach_proc =
                daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;
            let mut line_matcher = attach_proc.line_matcher()?;

            // generate a bunch of data that will cause the restore buffer to be too large
            // for a single chunk
            let blob = format!("echo {}", (0..max_chunk_size).map(|_| "x").collect::<String>());
            attach_proc.run_cmd(blob.as_str())?;
            line_matcher.match_re("xx$")?;

            attach_proc.run_cmd("echo food")?;
            line_matcher.match_re("food$")?;
        }

        // wait until the daemon has noticed that the connection
        // has dropped before we attempt to open the connection again
        daemon_proc.events = Some(bidi_done_w.wait_final_event("daemon-bidi-stream-done")?);

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
    })
}

#[test]
#[timeout(30000)]
fn exits_with_same_status_as_shell() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
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
    })
}

#[test]
#[timeout(30000)]
fn ttl_hangup() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut attach_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs { ttl: Some(time::Duration::from_secs(1)), ..Default::default() },
            )
            .context("starting attach proc")?;

        // ensure the shell is up and running
        let mut line_matcher = attach_proc.line_matcher()?;
        attach_proc.run_cmd("echo hi")?;
        line_matcher.match_re("hi$")?;

        // sleep long enough for the reaper to clobber the thread
        thread::sleep(time::Duration::from_millis(1200));

        let listout = daemon_proc.list()?;
        assert!(!String::from_utf8_lossy(listout.stdout.as_slice()).contains("sh1"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn ttl_no_hangup_yet() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new("norc.toml", true).context("starting daemon proc")?;
        let mut attach_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs { ttl: Some(time::Duration::from_secs(1000)), ..Default::default() },
            )
            .context("starting attach proc")?;

        // ensure the shell is up and running
        let mut line_matcher = attach_proc.line_matcher()?;
        attach_proc.run_cmd("echo hi")?;
        line_matcher.match_re("hi$")?;

        let listout = daemon_proc.list()?;
        assert!(String::from_utf8_lossy(listout.stdout.as_slice()).contains("sh1"));

        Ok(())
    })
}

#[ignore] // TODO: re-enable, this test if flaky
#[test]
fn up_arrow_no_crash() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml", false).context("starting daemon proc")?;
    let mut attach_proc =
        daemon_proc.attach("sh1", Default::default()).context("starting attach proc")?;

    let mut line_matcher = attach_proc.line_matcher()?;

    // Before we put the pty into raw mode, this would
    // cause crashes.
    attach_proc.run_raw_cmd(vec![27, 91, 65, 10])?; // up arrow
    line_matcher.match_re("logout$")?;

    Ok(())
}
