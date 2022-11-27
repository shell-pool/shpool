use std::path::{PathBuf, Path};
use std::process::{Command, Stdio};
use std::{process, time};
use std::os::unix::net::UnixStream;

use anyhow::{anyhow, Context};
use tempfile::TempDir;

use super::{events::Events, attach, shpool_bin, testdata_file};

/// Proc is a helper handle for a `shpool daemon` subprocess.
/// It kills the subprocess when it goes out of scope.
pub struct Proc {
    proc: process::Child,
    tmp_dir: Option<TempDir>,
    subproc_counter: usize,
    log_file: PathBuf,
    pub events: Option<Events>,
    pub socket_path: PathBuf,
}

impl Proc {
    pub fn new<P: AsRef<Path>>(config: P) -> anyhow::Result<Proc> {
        let tmp_dir = tempfile::Builder::new().prefix("shpool-test").rand_bytes(20)
            .tempdir().context("creating tmp dir")?;
        let socket_path = tmp_dir.path().join("shpool.socket");
        let test_hook_socket_path = tmp_dir.path().join("shpool-daemon-test-hook.socket");

        let log_file = tmp_dir.path().join("daemon.log");
        eprintln!("spawning daemon proc with log {:?}", &log_file);

        let proc = Command::new(shpool_bin())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-vv")
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

        Ok(Proc {
            proc,
            tmp_dir: Some(tmp_dir),
            log_file,
            subproc_counter: 0,
            events: Some(events),
            socket_path,
        })
    }

    pub fn attach(&mut self, name: &str) -> anyhow::Result<attach::Proc> {
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

        Ok(attach::Proc {
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

impl std::ops::Drop for Proc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing daemon proc: {:?}", e);
        }
        if std::env::var("SHPOOL_LEAVE_TEST_LOGS").unwrap_or(String::from("")) == "true" {
            self.tmp_dir.take().map(|d| d.into_path());
        }
    }
}
