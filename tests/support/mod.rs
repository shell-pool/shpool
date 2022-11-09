// This module is used from multiple different test files, each of which
// gets compiled into its own binary. Not all the binaries use all the
// stuff here.
#![allow(dead_code)]

use std::io::{BufRead, Write};
use std::path::{PathBuf, Path};
use std::process::{Command, Stdio};
use std::{env, process, time, io};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context};
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
    pub events: Option<Events>,
    pub socket_path: PathBuf,
}

impl DaemonProc {
    pub fn new<P: AsRef<Path>>(config: P) -> anyhow::Result<DaemonProc> {
        let tmp_dir = tempfile::Builder::new().prefix("shpool-test").rand_bytes(20)
            .tempdir().context("creating tmp dir")?;
        let socket_path = tmp_dir.path().join("shpool.socket");
        let test_hook_socket_path = tmp_dir.path().join("shpool-daemon-test-hook.socket");

        let log_file = tmp_dir.path().join("daemon.log");
        eprintln!("spawning daemon proc with log {:?}", &log_file);

        let proc = Command::new(shpool_bin())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-v")
            .arg("--log-file").arg(&log_file)
            .arg("--socket").arg(&socket_path)
            .arg("daemon")
            .arg("--config-file").arg(testdata_file(config))
            .env("SHPOOL_TEST_HOOK_SOCKET_PATH", &test_hook_socket_path)
            .spawn()
            .context("spawning daemon process")?;

        let events = Events::new(&test_hook_socket_path)?;

        // spin until we can dial the socket successfully
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
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
            events: Some(events),
            socket_path,
        })
    }

    pub fn attach(&mut self, name: &str) -> anyhow::Result<AttachProc> {
        let tmp_dir = self.tmp_dir.as_ref().ok_or(anyhow!("missing tmp_dir"))?;
        let log_file = tmp_dir.path().join(format!("attach_{}_{}.log", name, self.subproc_counter));
        let test_hook_socket_path = tmp_dir.path()
            .join(format!("attach_test_hook_{}_{}.socket", name, self.subproc_counter));
        eprintln!("spawning attach proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        let proc = Command::new(shpool_bin())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .arg("-v")
            .arg("--log-file").arg(&log_file)
            .arg("--socket").arg(&self.socket_path)
            .env("SHPOOL_TEST_HOOK_SOCKET_PATH", &test_hook_socket_path)
            .arg("attach")
            .arg(name)
            .spawn()
            .context(format!("spawning attach proc for {}", name))?;

        let events = Events::new(&test_hook_socket_path)?;

        Ok(AttachProc {
            proc,
            log_file,
            events: Some(events),
        })
    }

    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        if let Some(events) = &mut self.events {
            events.await_event(event)
        } else {
            Err(anyhow!("no events stream"))
        }
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
    pub events: Option<Events>,
}

const CMD_READ_TIMEOUT: time::Duration = time::Duration::from_secs(3);
const CMD_READ_SLEEP_DUR: time::Duration = time::Duration::from_millis(20);

impl AttachProc {
    pub fn run_cmd(
        &mut self,
        cmd: &str,
    ) -> anyhow::Result<()> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        let full_cmd = format!("{}\n", cmd);
        stdin.write_all(full_cmd.as_bytes()).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    /// Create a handle for asserting about output lines.
    ///
    /// For some reason we can't just create the Lines iterator as soon
    /// as we spawn the subcommand. Attempts to do so result in
    /// `Resource temporarily unavailable` (EAGAIN) errors.
    pub fn line_matcher(&mut self) -> anyhow::Result<LineMatcher> {
        let r = self.proc.stdout.take().ok_or(anyhow!("missing stdout"))?;

        nix::fcntl::fcntl(
            r.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        ).context("setting stdin nonblocking")?;

        let lines = io::BufReader::new(r).lines();
        Ok(LineMatcher{ out_lines: lines })
    }

    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        if let Some(events) = &mut self.events {
            events.await_event(event)
        } else {
            Err(anyhow!("no events stream"))
        }
    }
}

pub struct LineMatcher {
    out_lines: io::Lines<io::BufReader<process::ChildStdout>>,
}
impl LineMatcher {
    pub fn match_re(&mut self, re: &str) -> anyhow::Result<()> {
        let start = time::Instant::now();
        loop {
            let line = self.out_lines.next().ok_or(anyhow!("no line"))?;
            if let Err(e) = &line {
                if e.kind() == io::ErrorKind::WouldBlock {
                    if start.elapsed() > CMD_READ_TIMEOUT {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out reading line"))?;
                    }

                    std::thread::sleep(CMD_READ_SLEEP_DUR);
                    continue;
                }
            }
            let line = line?;

            eprintln!("testing /{}/ against '{}'", re, &line);
            return if Regex::new(re)?.is_match(&line) {
                Ok(())
            } else {
                Err(anyhow!("expected /{}/ to match '{}'", re, &line))
            };
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

/// Event represents a stream of events you can wait for.
///
/// To actually wait for a particular event, you should create
/// an EventWaiter with the `waiter` or `await_event` routines.
pub struct Events {
    lines: io::Lines<io::BufReader<UnixStream>>,
}

impl Events {
    fn new<P: AsRef<Path>>(sock: P) -> anyhow::Result<Self> {
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            if let Ok(s) = UnixStream::connect(&sock) {
                return Ok(Events {
                    lines: io::BufReader::new(s).lines(),
                });
            } else {
                std::thread::sleep(sleep_dur);
                sleep_dur *= 2;
            }
        }

        Err(anyhow!("timed out waiting for connection to event sock"))
    }

    /// waiter creates an event waiter that can later be used to
    /// block until the event occurs. You should generally call waiter
    /// before you take the action that will trigger the event in order
    /// to avoid race conditions.
    ///
    /// `events` should be a list of events to listen for, in order.
    /// You can wait for the events by calling methods on the EventWaiter,
    /// and you should make sure to use `wait_final_event` to get the
    /// Events struct back at the last event.
    pub fn waiter<S, SI>(mut self, events: SI) -> EventWaiter
        where S: Into<String>,
             SI: IntoIterator<Item = S> {
        let events: Vec<String> = events.into_iter().map(|s| s.into()).collect();
        assert!(events.len() > 0);

        let (tx, rx) = crossbeam_channel::bounded(events.len());
        let waiter = EventWaiter {
            matched: rx,
        };
        std::thread::spawn(move || {
            let mut return_lines = false;
            let mut offset = 0;

            'LINELOOP:
            for line in &mut self.lines {
                match line {
                    Ok(l) => {
                        if events[offset] == l {
                            if offset == events.len() - 1 {
                                // this is the last event
                                return_lines = true;
                                break 'LINELOOP;
                            } else {
                                tx.send(WaiterEvent::Event(l)).unwrap();
                            }
                            offset += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("error scanning for event '{}': {:?}", events[offset], e);
                    }
                }
            }

            if return_lines {
                tx.send(WaiterEvent::Done((events[offset].clone(), self.lines))).unwrap();
            }
        });

        waiter
    }

    /// await_events waits for a given event on the stream.
    /// Prefer `waiter` since it is less prone to race conditions.
    /// `await_event` might be approriate for startup events where
    /// it is not possible to use `waiter`.
    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        for line in &mut self.lines {
            let line = line?;
            if line == event {
                return Ok(());
            }
        }

        Ok(())
    }
}

/// EventWaiter represents waiting for a particular event.
/// It should be converted back into an Events struct with
/// the wait() routine.
pub struct EventWaiter {
    matched: crossbeam_channel::Receiver<WaiterEvent>,
}

enum WaiterEvent {
    Event(String),
    Done((String, io::Lines<io::BufReader<UnixStream>>)),
}

impl EventWaiter {
    pub fn wait_event(&mut self, event: &str) -> anyhow::Result<()> {
        match self.matched.recv()? {
            WaiterEvent::Event(e) => {
                return if e == event {
                    Ok(())
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                };
            },
            WaiterEvent::Done((e, _)) => {
                return if e == event {
                    Ok(())
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                };
            }
        }
    }

    pub fn wait_final_event(self, event: &str) -> anyhow::Result<Events> {
        match self.matched.recv()? {
            WaiterEvent::Event(e) => {
                Err(anyhow!("Got non-fianl '{}' event, want final '{}'", e, event))
            },
            WaiterEvent::Done((e, lines)) => {
                return if e == event {
                    Ok(Events {
                        lines
                    })
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                };
            }
        }
    }
}

