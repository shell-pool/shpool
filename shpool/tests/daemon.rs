use std::{
    fmt::Write,
    io::Read,
    os::unix::{
        io::{AsRawFd, FromRawFd},
        net::UnixListener,
        process::CommandExt,
    },
    path,
    process::{Command, Stdio},
    time,
};

use anyhow::{anyhow, Context};
use nix::{
    sys::signal::{self, Signal},
    unistd::{ForkResult, Pid},
};
use ntest::timeout;
use regex::Regex;

mod support;

use crate::support::daemon::{AttachArgs, DaemonArgs};

#[test]
#[timeout(30000)]
fn start() -> anyhow::Result<()> {
    support::dump_err(|| {
        let tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;

        let mut child = Command::new(support::shpool_bin()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("--socket")
            .arg(tmp_dir.path().join("shpool.socket"))
            .arg("daemon")
            .spawn()
            .context("spawning daemon process")?;

        // The server should start up and run without incident for
        // half a second.
        std::thread::sleep(time::Duration::from_millis(500));

        child.kill().context("killing child")?;

        let mut stdout = child.stdout.take().context("missing stdout")?;
        let mut stdout_str = String::from("");
        stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;

        if !stdout_str.is_empty() {
            println!("{}", stdout_str);
            return Err(anyhow!("unexpected stdout output"));
        }

        let mut stderr = child.stderr.take().context("missing stderr")?;
        let mut stderr_str = String::from("");
        stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
        assert!(stderr_str.contains("STARTING DAEMON"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn systemd_activation() -> anyhow::Result<()> {
    support::dump_err(|| {
        let tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;
        let sock_path = tmp_dir.path().join("shpool.socket");
        let activation_sock = UnixListener::bind(&sock_path)?;

        let (parent_stderr, child_stderr) =
            nix::unistd::pipe().context("creating pipe to collect stderr")?;
        // Safety: this is a test
        let child_stderr_pipe = unsafe { Stdio::from_raw_fd(child_stderr) };
        let mut cmd = Command::new(support::shpool_bin()?);
        cmd.stdout(Stdio::piped())
            .stderr(child_stderr_pipe)
            .env("LISTEN_FDS", "1")
            .env("LISTEN_FDNAMES", sock_path)
            .arg("daemon");

        let mut pid_buf = String::with_capacity(128);

        // We use fork both so we can correctly set LISTEN_PID and so
        // that the daemon will inherit the socket fd the way that we
        // want.
        //
        // We have to manually fork rather than using pre_exec because
        // there does not appear to be a way to set an environment
        // variable the child will inherit in the pre_exec callback.
        //
        // Safety: it's a test, get off my back. I try to avoid allocating.
        let child_pid = match unsafe { nix::unistd::fork() } {
            Ok(ForkResult::Parent { child, .. }) => child,
            Ok(ForkResult::Child) => {
                // place the unix socket file descriptor in the right
                // place
                let fdarg = match nix::unistd::dup2(activation_sock.as_raw_fd(), 3) {
                    Ok(newfd) => newfd,
                    Err(e) => {
                        eprintln!("dup err: {}", e);
                        std::process::exit(1)
                    }
                };

                // unset the fd_cloexec flag on the file descriptor so
                // we can actuall pass it down to the child
                let fdflags = nix::fcntl::fcntl(fdarg, nix::fcntl::FcntlArg::F_GETFD)
                    .expect("getfd flags to work");
                let mut newflags = nix::fcntl::FdFlag::from_bits(fdflags).unwrap();
                newflags.remove(nix::fcntl::FdFlag::FD_CLOEXEC);
                nix::fcntl::fcntl(fdarg, nix::fcntl::FcntlArg::F_SETFD(newflags))
                    .expect("FD_CLOEXEC to be unset");

                // set the LISTEN_PID environment variable without
                // allocating
                write!(&mut pid_buf, "{}", std::process::id())
                    .expect("to be able to format the pid");
                cmd.env("LISTEN_PID", pid_buf);

                let err = cmd.exec();
                eprintln!("exec err: {:?}", err);
                std::process::exit(1);
            }
            Err(e) => {
                return Err(e).context("forking daemon proc");
            }
        };

        // The server should start up and run without incident for
        // half a second.
        std::thread::sleep(time::Duration::from_millis(500));

        // kill the daemon proc and reap the return code
        nix::sys::signal::kill(child_pid, Some(nix::sys::signal::Signal::SIGKILL))
            .context("killing daemon")?;
        nix::sys::wait::waitpid(child_pid, None).context("reaping daemon")?;

        let mut stderr_buf: Vec<u8> = vec![0; 1024 * 8];
        let len =
            nix::unistd::read(parent_stderr, &mut stderr_buf[..]).context("reading stderr")?;
        let stderr = String::from_utf8_lossy(&stderr_buf[..len]);
        assert!(stderr.contains("using systemd activation socket"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn config() -> anyhow::Result<()> {
    support::dump_err(|| {
        let tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;

        let mut child = Command::new(support::shpool_bin()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("--socket")
            .arg(tmp_dir.path().join("shpool.socket"))
            .arg("--config-file")
            .arg(support::testdata_file("empty.toml"))
            .arg("daemon")
            .spawn()
            .context("spawning daemon process")?;

        // The server should start up and run without incident for
        // half a second.
        std::thread::sleep(time::Duration::from_millis(500));

        child.kill().context("killing child")?;

        let mut stdout = child.stdout.take().context("missing stdout")?;
        let mut stdout_str = String::from("");
        stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;

        if !stdout_str.is_empty() {
            println!("{}", stdout_str);
            return Err(anyhow!("unexpected stdout output"));
        }

        let mut stderr = child.stderr.take().context("missing stderr")?;
        let mut stderr_str = String::from("");
        stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
        assert!(stderr_str.contains("STARTING DAEMON"));

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn hooks() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc =
            support::daemon::Proc::new_instrumented("norc.toml").context("starting daemon proc")?;
        let sh1_detached_re = Regex::new("sh1.*disconnected")?;

        {
            // 1 new session
            let mut sh1_proc = daemon_proc
                .attach(
                    "sh1",
                    AttachArgs { cmd: Some(String::from("/bin/bash")), ..Default::default() },
                )
                .context("starting attach proc")?;

            // sequencing
            let mut sh1_matcher = sh1_proc.line_matcher()?;
            sh1_proc.run_cmd("echo hi")?;
            sh1_matcher.scan_until_re("hi$")?;

            // 1 busy
            let mut busy_proc = daemon_proc
                .attach(
                    "sh1",
                    AttachArgs { cmd: Some(String::from("/bin/bash")), ..Default::default() },
                )
                .context("starting attach proc")?;
            busy_proc.proc.wait()?;
        } // 1 client disconnect

        // spin until sh1 disconnects
        daemon_proc.wait_until_list_matches(|listout| sh1_detached_re.is_match(listout))?;

        // 1 reattach
        let mut sh1_proc = daemon_proc
            .attach(
                "sh1",
                AttachArgs { cmd: Some(String::from("/bin/bash")), ..Default::default() },
            )
            .context("starting attach proc")?;
        sh1_proc.run_cmd("exit")?; // 1 shell disconnect

        support::wait_until(|| {
            let hook_records = daemon_proc.hook_records.as_ref().unwrap().lock().unwrap();
            Ok(!hook_records.shell_disconnects.is_empty())
        })?;

        let hook_records = daemon_proc.hook_records.as_ref().unwrap().lock().unwrap();
        eprintln!("hook_records: {:?}", hook_records);
        assert_eq!(hook_records.new_sessions[0], "sh1");
        assert_eq!(hook_records.reattaches[0], "sh1");
        assert_eq!(hook_records.busys[0], "sh1");
        assert_eq!(hook_records.client_disconnects[0], "sh1");
        assert_eq!(hook_records.shell_disconnects[0], "sh1");

        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn cleanup_socket() -> anyhow::Result<()> {
    support::dump_err(|| {
        let mut daemon_proc = support::daemon::Proc::new(
            "norc.toml",
            DaemonArgs { listen_events: false, ..DaemonArgs::default() },
        )
        .context("starting daemon proc")?;

        signal::kill(
            Pid::from_raw(daemon_proc.proc.as_ref().unwrap().id() as i32),
            Signal::SIGINT,
        )?;

        daemon_proc.proc_wait()?;

        assert!(!path::Path::new(&daemon_proc.socket_path).exists());
        Ok(())
    })
}

#[test]
#[timeout(30000)]
fn echo_sentinel() -> anyhow::Result<()> {
    support::dump_err(|| {
        let output = Command::new(support::shpool_bin()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("SHPOOL__PRINT_PROMPT_SETUP_SENTINEL", "yes")
            .arg("daemon")
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("SHPOOL_PROMPT_SETUP_SENTINEL"));

        Ok(())
    })
}
