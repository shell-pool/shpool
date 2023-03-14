use std::{
    fmt::Write,
    io::Read,
    os::unix::{
        io::{
            AsRawFd,
            FromRawFd,
        },
        net::UnixListener,
        process::CommandExt,
    },
    path,
    process::{
        Command,
        Stdio,
    },
    time,
};

use anyhow::{
    anyhow,
    Context,
};
use nix::{
    sys::signal::{
        self,
        Signal,
    },
    unistd::{
        ForkResult,
        Pid,
    },
};

mod support;

#[test]
fn start() -> anyhow::Result<()> {
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
    stdout
        .read_to_string(&mut stdout_str)
        .context("slurping stdout")?;

    if stdout_str != "" {
        println!("{}", stdout_str);
        return Err(anyhow!("unexpected stdout output"));
    }

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr
        .read_to_string(&mut stderr_str)
        .context("slurping stderr")?;
    assert!(stderr_str.contains("STARTING DAEMON"));

    Ok(())
}

#[test]
fn systemd_activation() -> anyhow::Result<()> {
    let tmp_dir = tempfile::Builder::new()
        .prefix("shpool-test")
        .rand_bytes(20)
        .tempdir()
        .context("creating tmp dir")?;
    let sock_path = tmp_dir.path().join("shpool.socket");
    let activation_sock = UnixListener::bind(&sock_path)?;

    let (parent_stderr, child_stderr) =
        nix::unistd::pipe().context("creating pipe to collect stderr")?;
    // Saftey: this is a test
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
    // Saftey: it's a test, get off my back. I try to avoid allocating.
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
                },
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
            write!(&mut pid_buf, "{}", std::process::id()).expect("to be able to format the pid");
            cmd.env("LISTEN_PID", pid_buf);

            let err = cmd.exec();
            ();
            eprintln!("exec err: {:?}", err);
            std::process::exit(1);
        },
        Err(e) => {
            return Err(e).context("forking daemon proc");
        },
    };

    // The server should start up and run without incident for
    // half a second.
    std::thread::sleep(time::Duration::from_millis(500));

    // kill the daemon proc and reap the return code
    nix::sys::signal::kill(child_pid, Some(nix::sys::signal::Signal::SIGKILL))
        .context("killing daemon")?;
    nix::sys::wait::waitpid(child_pid, None).context("reaping daemon")?;

    let mut stderr_buf: Vec<u8> = vec![0; 1024 * 8];
    let len = nix::unistd::read(parent_stderr, &mut stderr_buf[..]).context("reading stderr")?;
    let stderr = String::from_utf8_lossy(&stderr_buf[..len]);
    assert!(stderr.contains("using systemd activation socket"));

    Ok(())
}

#[test]
fn config() -> anyhow::Result<()> {
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
        .arg("--config-file")
        .arg(support::testdata_file("empty.toml"))
        .spawn()
        .context("spawning daemon process")?;

    // The server should start up and run without incident for
    // half a second.
    std::thread::sleep(time::Duration::from_millis(500));

    child.kill().context("killing child")?;

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout
        .read_to_string(&mut stdout_str)
        .context("slurping stdout")?;

    if stdout_str != "" {
        println!("{}", stdout_str);
        return Err(anyhow!("unexpected stdout output"));
    }

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr
        .read_to_string(&mut stderr_str)
        .context("slurping stderr")?;
    assert!(stderr_str.contains("STARTING DAEMON"));

    Ok(())
}

#[test]
fn cleanup_socket() -> anyhow::Result<()> {
    let mut daemon_proc =
        support::daemon::Proc::new("norc.toml").context("starting daemon proc")?;

    signal::kill(Pid::from_raw(daemon_proc.proc.id() as i32), Signal::SIGINT)?;

    daemon_proc.proc.wait()?;

    assert!(!path::Path::new(&daemon_proc.socket_path).exists());
    Ok(())
}
