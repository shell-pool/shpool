// This module is used from multiple different test files, each of which
// gets compiled into its own binary. Not all the binaries use all the
// stuff here.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::{PathBuf, Path};
use std::process::{Command, Stdio};
use std::{env, process, time, io};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context};
use bitflags::bitflags;
use tempfile::TempDir;
use regex::Regex;

pub fn testdata_file<P: AsRef<Path>>(file: P) -> PathBuf {
    let mut dir = cargo_dir();
    dir.pop();
    dir.pop();
    dir.join("tests").join("data").join(file)
}

pub fn shpool_bin() -> PathBuf {
    cargo_dir().join("shpool")
}

pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH").map(PathBuf::from).or_else(|| {
        env::current_exe().ok().map(|mut path| {
            path.pop();
            if path.ends_with("deps") {
                path.pop();
            }
            path
        })
    }).unwrap_or_else(|| {
        panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
    })
}

/// DaemonProc is a helper handle for a `shpool daemon` subprocess. It kills the
/// subprocess when it goes out of scope.
pub struct DaemonProc {
    proc: process::Child,
    tmp_dir: Option<TempDir>,
    subproc_counter: usize,
    log_file: PathBuf,
    pub socket_path: PathBuf,
}

impl DaemonProc {
    pub fn new<P: AsRef<Path>>(config: P) -> anyhow::Result<DaemonProc> {
        let tmp_dir = tempfile::Builder::new().prefix("shpool-test").rand_bytes(20)
            .tempdir().context("creating tmp dir")?;
        let socket_path = tmp_dir.path().join("shpool.socket");

        let log_file = tmp_dir.path().join("daemon.log");

        let proc = Command::new(shpool_bin())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-v")
            .arg("--log-file").arg(&log_file)
            .arg("--socket").arg(&socket_path)
            .arg("daemon")
            .arg("--config-file").arg(testdata_file(config))
            .spawn()
            .context("spawning daemon process")?;

        // spin until we can dial the socket successfully
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..30 {
            if let Ok(_) = UnixStream::connect(&socket_path) {
                break;
            } else {
                std::thread::sleep(sleep_dur);
                sleep_dur *= 2;
            }
        }

        Ok(DaemonProc {
            proc,
            tmp_dir: Some(tmp_dir),
            log_file,
            subproc_counter: 0,
            socket_path,
        })
    }

    pub fn attach(&mut self, name: &str) -> anyhow::Result<AttachProc> {
        let tmp_dir =  self.tmp_dir.as_ref().ok_or(anyhow!("missing tmp_dir"))?;
        let log_file = tmp_dir.path().join(format!("attach_{}_{}.log", name, self.subproc_counter));
        self.subproc_counter += 1;

        let proc = Command::new(shpool_bin())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .arg("-v")
            .arg("--log-file").arg(&log_file)
            .arg("--socket").arg(&self.socket_path)
            .arg("attach")
            .arg(name)
            .spawn()
            .context(format!("spawning attach proc for {}", name))?;

        let stdout = self.proc.stdout.as_ref().ok_or(anyhow!("missing stdout"))?;
        let stderr = self.proc.stderr.as_mut().ok_or(anyhow!("missing stderr"))?;

        for fd in vec![stdout.as_raw_fd(), stderr.as_raw_fd()].iter() {
            nix::fcntl::fcntl(
                *fd,
                nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
            ).context("setting stdin nonblocking")?;
        }

        Ok(AttachProc {
            proc,
            log_file,
        })
    }
}

impl std::ops::Drop for DaemonProc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing daemon proc: {:?}", e);
        }
        if std::env::var("SHPOOL_LEAVE_TEST_LOGS").unwrap_or(String::from("")) == "true" {
            self.tmp_dir.take().map(|d| d.keep());
        }
    }
}

/// AttachProc is a handle for a `shpool attach` subprocess spawned for testing
pub struct AttachProc {
    proc: process::Child,
    log_file: PathBuf,
}

const CMD_READ_LONG_TIMEOUT: time::Duration = time::Duration::from_secs(1);
const CMD_READ_SHORT_TIMEOUT: time::Duration = time::Duration::from_millis(200);
const CMD_READ_SLEEP_DUR: time::Duration = time::Duration::from_millis(20);

bitflags! {
    pub struct ExpectedOutput: u32 {
        const STDOUT = 0b00000001;
        const STDERR = 0b00000010;
    }
}

impl AttachProc {
    // TODO(ethan): have caller specify the channels they expect output on
    // (stdin/stdout/none) and block with a long timeout for those channels
    pub fn run_cmd(
        &mut self,
        cmd: &str,
        expected_outputs: ExpectedOutput,
    ) -> anyhow::Result<CmdResult> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;
        let stdout = self.proc.stdout.as_mut().ok_or(anyhow!("missing stdout"))?;
        let stderr = self.proc.stderr.as_mut().ok_or(anyhow!("missing stderr"))?;

        let full_cmd = format!("{}\n", cmd);
        stdin.write_all(full_cmd.as_bytes()).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        let mut buf: [u8; 1024*4] = [0; 1024*4];


        let nbytes = match read_chunk(stdout, &mut buf[..], expected_outputs.contains(ExpectedOutput::STDOUT)) {
            Ok(n) => n,
            Err(e) => {
                if let Some(io_err) = e.downcast_ref::<io::Error>() {
                    if io_err.kind() == io::ErrorKind::TimedOut {
                        0 // we will just move on with the empty string
                    } else {
                        Err(e).context("reading stdout chunk")?
                    }
                } else {
                    Err(e).context("reading stdout chunk")?
                }
            },
        };
        let stdout_str = String::from(String::from_utf8_lossy(&buf[..nbytes]));

        let nbytes = match read_chunk(stderr, &mut buf[..], expected_outputs.contains(ExpectedOutput::STDERR)) {
            Ok(n) => n,
            Err(e) => {
                if let Some(io_err) = e.downcast_ref::<io::Error>() {
                    if io_err.kind() == io::ErrorKind::TimedOut {
                        0 // we will just move on with the empty string
                    } else {
                        Err(e).context("reading stderr chunk")?
                    }
                } else {
                    Err(e).context("reading stderr chunk")?
                }
            },
        };
        let stderr_str = String::from(String::from_utf8_lossy(&buf[..nbytes]));

        Ok(CmdResult {
            stdout_str,
            stderr_str,
        })
    }
}

fn read_chunk<R>(r: &mut R, into: &mut [u8], expect_output: bool) -> anyhow::Result<usize>
    where R: Read + AsRawFd
{
    let timeout = if expect_output {
        CMD_READ_LONG_TIMEOUT
    } else {
        CMD_READ_SHORT_TIMEOUT
    };

    let start = time::Instant::now();
    loop {
        let mut timeout_tv = nix::sys::time::TimeVal::new(
            0, 1000 * (timeout.as_millis() as nix::sys::time::suseconds_t));

        let mut fdset = nix::sys::select::FdSet::new();
        fdset.insert(r.as_raw_fd());
        let nready = nix::sys::select::select(
            None,
            Some(&mut fdset),
            None,
            None,
            Some(&mut timeout_tv),
        ).context("selecting on stdout")?;

        if nready == 0 || !fdset.contains(r.as_raw_fd()) {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out reading chunk"))?;
        }

        let nbytes = r.read(into)?;
        if nbytes == 0 {
            if start.elapsed() > timeout {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out reading chunk"))?;
            } else {
                std::thread::sleep(CMD_READ_SLEEP_DUR);
            }
        } else {
            return Ok(nbytes);
        }
    }
}

impl std::ops::Drop for AttachProc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing attach proc: {:?}", e);
        }
    }
}

/// CmdResult represents a command that has been run as part of a subshell. All the
/// output is slurped ahead of time and there are some helper routines for making
/// assertions about it.
pub struct CmdResult {
    stdout_str: String,
    stderr_str: String,
}

impl CmdResult {
    pub fn stdout_re_match(&self, pat: &str) -> anyhow::Result<bool> {
        Ok(Regex::new(pat)?.is_match(&self.stdout_str))
    }

    pub fn stderr_re_match(&self, pat: &str) -> anyhow::Result<bool> {
        Ok(Regex::new(pat)?.is_match(&self.stderr_str))
    }
}
