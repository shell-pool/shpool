use std::{
    default::Default,
    env,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process,
    process::{Command, Stdio},
    time,
};

use anyhow::{anyhow, Context};
use rand::Rng;
use tempfile::TempDir;

use super::{attach, events::Events, shpool_bin, testdata_file};

/// Proc is a helper handle for a `shpool daemon` subprocess.
/// It kills the subprocess when it goes out of scope.
pub struct Proc {
    pub proc: process::Child,
    subproc_counter: usize,
    log_file: PathBuf,
    local_tmp_dir: Option<TempDir>,
    pub tmp_dir: PathBuf,
    pub events: Option<Events>,
    pub socket_path: PathBuf,
}

pub struct AttachArgs {
    pub force: bool,
    pub extra_env: Vec<(String, String)>,
    pub ttl: Option<time::Duration>,
}

impl Default for AttachArgs {
    fn default() -> Self {
        AttachArgs { force: false, extra_env: vec![], ttl: None }
    }
}

impl Proc {
    pub fn new<P: AsRef<Path>>(config: P, listen_events: bool) -> anyhow::Result<Proc> {
        let local_tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;
        let tmp_dir = if let Ok(base) = std::env::var("KOKORO_ARTIFACTS_DIR") {
            let mut dir = PathBuf::from(base);
            let rand_blob: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(20)
                .map(char::from)
                .collect();
            dir.push(format!("shpool-test{}", rand_blob));
            std::fs::create_dir(&dir)?;
            dir
        } else {
            local_tmp_dir.path().to_path_buf()
        };

        let socket_path = tmp_dir.join("shpool.socket");
        let test_hook_socket_path = tmp_dir.join("shpool-daemon-test-hook.socket");

        let log_file = tmp_dir.join("daemon.log");
        eprintln!("spawning daemon proc with log {:?}", &log_file);

        let mut cmd = Command::new(shpool_bin()?);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-vv")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&socket_path)
            .arg("daemon")
            .arg("--config-file")
            .arg(testdata_file(config));
        if listen_events {
            cmd.env("SHPOOL_TEST_HOOK_SOCKET_PATH", &test_hook_socket_path);
        }
        let proc = cmd.spawn().context("spawning daemon process")?;

        let events = if listen_events { Some(Events::new(&test_hook_socket_path)?) } else { None };

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

        Ok(Proc {
            proc,
            local_tmp_dir: Some(local_tmp_dir),
            tmp_dir,
            log_file,
            subproc_counter: 0,
            events,
            socket_path,
        })
    }

    pub fn attach(&mut self, name: &str, args: AttachArgs) -> anyhow::Result<attach::Proc> {
        let log_file = self.tmp_dir.join(format!("attach_{}_{}.log", name, self.subproc_counter));
        let test_hook_socket_path =
            self.tmp_dir.join(format!("attach_test_hook_{}_{}.socket", name, self.subproc_counter));
        eprintln!("spawning attach proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        let mut cmd = Command::new(shpool_bin()?);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .arg("-v")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&self.socket_path)
            .env_clear()
            .env("XDG_RUNTIME_DIR", env::var("XDG_RUNTIME_DIR")?)
            .env("SHPOOL_TEST_HOOK_SOCKET_PATH", &test_hook_socket_path)
            .envs(args.extra_env)
            .arg("attach");
        if args.force {
            cmd.arg("-f");
        }
        if let Some(ttl) = args.ttl {
            cmd.arg("--ttl");
            cmd.arg(format!("{}s", ttl.as_secs()));
        }
        let proc = cmd.arg(name).spawn().context(format!("spawning attach proc for {}", name))?;

        let events = Events::new(&test_hook_socket_path)?;

        Ok(attach::Proc { proc, log_file, events: Some(events) })
    }

    pub fn detach(&mut self, sessions: Vec<String>) -> anyhow::Result<process::Output> {
        let log_file = self.tmp_dir.join(format!("detach_{}.log", self.subproc_counter));
        eprintln!("spawning detach proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        let mut cmd = Command::new(shpool_bin()?);
        cmd.arg("-vv")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&self.socket_path)
            .arg("detach");
        for session in sessions.iter() {
            cmd.arg(session);
        }

        cmd.output().context("spawning detach proc")
    }

    pub fn kill(&mut self, sessions: Vec<String>) -> anyhow::Result<process::Output> {
        let log_file = self.tmp_dir.join(format!("kill_{}.log", self.subproc_counter));
        eprintln!("spawning kill proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        let mut cmd = Command::new(shpool_bin()?);
        cmd.arg("-vv")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&self.socket_path)
            .arg("kill");
        for session in sessions.iter() {
            cmd.arg(session);
        }

        cmd.output().context("spawning kill proc")
    }

    /// list launches a `shpool list` process, collects the
    /// output and returns it as a string
    pub fn list(&mut self) -> anyhow::Result<process::Output> {
        let log_file = self.tmp_dir.join(format!("list_{}.log", self.subproc_counter));
        eprintln!("spawning list proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        Command::new(shpool_bin()?)
            .arg("-vv")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&self.socket_path)
            .arg("list")
            .output()
            .context("spawning list proc")
    }

    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        if let Some(events) = &mut self.events {
            events.await_event(event)
        } else {
            Err(anyhow!("no events stream"))
        }
    }
}

impl std::ops::Drop for Proc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing daemon proc: {:?}", e);
        }
        if std::env::var("SHPOOL_LEAVE_TEST_LOGS").unwrap_or(String::from("")) == "true" {
            self.local_tmp_dir.take().map(|d| d.into_path());
        }
    }
}
