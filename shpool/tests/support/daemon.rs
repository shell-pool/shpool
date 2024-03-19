use std::{
    default::Default,
    env,
    os::unix::{net::UnixStream, prelude::ExitStatusExt},
    path::{Path, PathBuf},
    process,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread, time,
};

use anyhow::{anyhow, Context};
use tempfile::TempDir;

use super::{attach, events::Events, shpool_bin, testdata_file, wait_until};

/// Proc is a helper handle for a `shpool daemon` subprocess.
/// It kills the subprocess when it goes out of scope.
pub struct Proc {
    pub proc: Option<process::Child>,
    subproc_counter: usize,
    log_file: PathBuf,
    local_tmp_dir: Option<TempDir>,
    pub tmp_dir: PathBuf,
    pub events: Option<Events>,
    pub socket_path: PathBuf,
    // Only present when created by new_instrumented()
    pub hook_records: Option<Arc<Mutex<HookRecords>>>,
}

#[derive(Default)]
pub struct AttachArgs {
    pub config: Option<String>,
    pub force: bool,
    pub extra_env: Vec<(String, String)>,
    pub ttl: Option<time::Duration>,
    pub cmd: Option<String>,
}

pub struct HooksRecorder {
    records: Arc<Mutex<HookRecords>>,
}

impl libshpool::Hooks for HooksRecorder {
    fn on_new_session(&self, session_name: &str) -> anyhow::Result<()> {
        eprintln!("on_new_session: {}", session_name);
        let mut recs = self.records.lock().unwrap();
        recs.new_sessions.push(String::from(session_name));
        Ok(())
    }

    fn on_reattach(&self, session_name: &str) -> anyhow::Result<()> {
        eprintln!("on_reattach: {}", session_name);
        let mut recs = self.records.lock().unwrap();
        recs.reattaches.push(String::from(session_name));
        Ok(())
    }

    fn on_busy(&self, session_name: &str) -> anyhow::Result<()> {
        eprintln!("on_busy: {}", session_name);
        let mut recs = self.records.lock().unwrap();
        recs.busys.push(String::from(session_name));
        Ok(())
    }

    fn on_client_disconnect(&self, session_name: &str) -> anyhow::Result<()> {
        eprintln!("on_client_disconnect: {}", session_name);
        let mut recs = self.records.lock().unwrap();
        recs.client_disconnects.push(String::from(session_name));
        Ok(())
    }

    fn on_shell_disconnect(&self, session_name: &str) -> anyhow::Result<()> {
        eprintln!("on_shell_disconnect: {}", session_name);
        let mut recs = self.records.lock().unwrap();
        recs.shell_disconnects.push(String::from(session_name));
        Ok(())
    }
}

#[derive(Debug)]
pub struct HookRecords {
    pub new_sessions: Vec<String>,
    pub reattaches: Vec<String>,
    pub busys: Vec<String>,
    pub client_disconnects: Vec<String>,
    pub shell_disconnects: Vec<String>,
}

impl Proc {
    pub fn new<P: AsRef<Path>>(config: P, listen_events: bool) -> anyhow::Result<Proc> {
        let local_tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;
        let tmp_dir = local_tmp_dir.path().to_path_buf();

        let socket_path = tmp_dir.join("shpool.socket");
        let test_hook_socket_path = tmp_dir.join("shpool-daemon-test-hook.socket");

        let log_file = tmp_dir.join("daemon.log");
        eprintln!("spawning daemon proc with log {:?}", &log_file);

        let resolved_config = if config.as_ref().exists() {
            PathBuf::from(config.as_ref())
        } else {
            testdata_file(config)
        };

        let mut cmd = Command::new(shpool_bin()?);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-vv")
            .arg("--log-file")
            .arg(&log_file)
            .arg("--socket")
            .arg(&socket_path)
            .arg("--config-file")
            .arg(resolved_config)
            .arg("daemon");
        if listen_events {
            cmd.env("SHPOOL_TEST_HOOK_SOCKET_PATH", &test_hook_socket_path);
        }
        let proc = cmd.spawn().context("spawning daemon process")?;

        let events = if listen_events { Some(Events::new(&test_hook_socket_path)?) } else { None };

        // spin until we can dial the socket successfully
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            if UnixStream::connect(&socket_path).is_ok() {
                break;
            } else {
                std::thread::sleep(sleep_dur);
                sleep_dur *= 2;
            }
        }

        Ok(Proc {
            proc: Some(proc),
            local_tmp_dir: Some(local_tmp_dir),
            tmp_dir,
            log_file,
            subproc_counter: 0,
            events,
            socket_path,
            hook_records: None,
        })
    }

    // Start a daemon process using a background thread rather than forking an
    // actual subprocess. Include a custom hooks impl for tracking events.
    pub fn new_instrumented<P: AsRef<Path>>(config: P) -> anyhow::Result<Proc> {
        let local_tmp_dir = tempfile::Builder::new()
            .prefix("shpool-test")
            .rand_bytes(20)
            .tempdir()
            .context("creating tmp dir")?;
        let tmp_dir = local_tmp_dir.path().to_path_buf();

        let socket_path = tmp_dir.join("shpool.socket");

        let log_file = tmp_dir.join("daemon.log");
        eprintln!("spawning instrumented daemon thread with log {:?}", &log_file);

        let args = libshpool::Args {
            log_file: Some(
                log_file
                    .clone()
                    .into_os_string()
                    .into_string()
                    .map_err(|e| anyhow!("conversion error: {:?}", e))?,
            ),
            verbose: 2,
            socket: Some(
                socket_path
                    .clone()
                    .into_os_string()
                    .into_string()
                    .map_err(|e| anyhow!("conversion error: {:?}", e))?,
            ),
            config_file: Some(
                testdata_file(config)
                    .into_os_string()
                    .into_string()
                    .map_err(|e| anyhow!("conversion error: {:?}", e))?,
            ),
            command: libshpool::Commands::Daemon,
        };
        let hooks_recorder = Box::new(HooksRecorder {
            records: Arc::new(Mutex::new(HookRecords {
                new_sessions: vec![],
                reattaches: vec![],
                busys: vec![],
                client_disconnects: vec![],
                shell_disconnects: vec![],
            })),
        });
        let hook_records = Arc::clone(&hooks_recorder.records);

        // spawn the daemon in a thread
        thread::spawn(move || {
            if let Err(err) = libshpool::run(args, Some(hooks_recorder)) {
                eprintln!("shpool proc exited with err: {:?}", err);
            }
        });

        // spin until we can dial the socket successfully
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            if UnixStream::connect(&socket_path).is_ok() {
                break;
            } else {
                std::thread::sleep(sleep_dur);
                sleep_dur *= 2;
            }
        }

        Ok(Proc {
            proc: None,
            local_tmp_dir: Some(local_tmp_dir),
            tmp_dir,
            log_file,
            subproc_counter: 0,
            events: None,
            socket_path,
            hook_records: Some(hook_records),
        })
    }

    pub fn proc_kill(&mut self) -> std::io::Result<()> {
        if let Some(proc) = &mut self.proc { proc.kill() } else { Ok(()) }
    }

    pub fn proc_wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        if let Some(proc) = &mut self.proc {
            proc.wait()
        } else {
            Ok(process::ExitStatus::from_raw(0))
        }
    }

    pub fn attach(&mut self, name: &str, args: AttachArgs) -> anyhow::Result<attach::Proc> {
        let log_file = self.tmp_dir.join(format!("attach_{}_{}.log", name, self.subproc_counter));
        let test_hook_socket_path =
            self.tmp_dir.join(format!("attach_test_hook_{}_{}.socket", name, self.subproc_counter));
        eprintln!("spawning attach proc with log {:?}", &log_file);
        self.subproc_counter += 1;

        let mut cmd = Command::new(shpool_bin()?);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
        if let Some(config_file) = args.config {
            cmd.arg("--config-file").arg(testdata_file(config_file));
        }
        cmd.arg("-v")
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
        if let Some(cmd_str) = &args.cmd {
            cmd.arg("-c");
            cmd.arg(cmd_str);
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

    pub fn wait_until_list_matches<F>(&mut self, pred: F) -> anyhow::Result<()>
    where
        F: Fn(&str) -> bool,
    {
        wait_until(|| {
            let list_out = self.list()?;
            if !list_out.status.success() {
                let list_stderr = String::from_utf8_lossy(&list_out.stdout[..]);
                eprintln!("list bad exit, stderr: {:?}", list_stderr);
                return Ok(false);
            }
            let list_stdout = String::from_utf8_lossy(&list_out.stdout[..]);
            Ok(pred(&list_stdout))
        })
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
        if let Err(e) = self.proc_kill() {
            eprintln!("err killing daemon proc: {:?}", e);
        }
        if std::env::var("SHPOOL_LEAVE_TEST_LOGS").unwrap_or(String::from("")) == "true" {
            self.local_tmp_dir.take().map(|d| d.into_path());
        }
    }
}
