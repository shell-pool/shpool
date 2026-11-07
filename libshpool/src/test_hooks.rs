// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// tooling gets confused by the conditional compilation
#![allow(dead_code)]

// The test_hooks module provides a mechanism for exposing events to
// the test harness so that it does not have to rely on buggy and slow
// sleeps in order to test various scenarios. The basic idea is that
// we publish a unix socket and then clients can listen for specific
// named events in order to block until they have occurred.
use std::{
    collections::HashSet,
    io::{BufRead, Write},
    os::unix::net::{UnixListener, UnixStream},
    thread, time,
};

use anyhow::{anyhow, Context};
use parking_lot::{Condvar, Mutex};
use tracing::{error, info};

#[cfg(feature = "test_hooks")]
pub fn emit(event: &str) {
    let has_sock = TEST_HOOK_SERVER.sock_path.lock().is_some();
    if has_sock {
        TEST_HOOK_SERVER.emit_event(event);
        TEST_HOOK_SERVER.maybe_pause(event);
    }
}

#[cfg(not(feature = "test_hooks"))]
pub fn emit(_event: &str) {
    // a no-op normally
}

/// Consume a named failure point at most once per process. Configure points
/// with a comma-separated SHPOOL_TEST__FAILURE_POINTS environment variable.
#[cfg(feature = "test_hooks")]
pub fn fail_once(point: &str) -> bool {
    use std::sync::OnceLock;

    static FAILURE_POINTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let points = FAILURE_POINTS.get_or_init(|| {
        Mutex::new(
            std::env::var("SHPOOL_TEST__FAILURE_POINTS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|point| !point.is_empty())
                .map(String::from)
                .collect(),
        )
    });
    let injected = points.lock().remove(point);
    if injected {
        emit(&format!("injected-failure-{point}"));
    }
    injected
}

#[cfg(not(feature = "test_hooks"))]
pub fn fail_once(_point: &str) -> bool {
    false
}

#[cfg(feature = "test_hooks")]
pub fn scoped(event: &str) -> ScopedEvent {
    ScopedEvent::new(event)
}

#[cfg(not(feature = "test_hooks"))]
pub fn scoped(_event: &str) {}

/// ScopedEvent emits an event when it goes out of scope
pub struct ScopedEvent<'a> {
    event: &'a str,
}

impl<'a> ScopedEvent<'a> {
    pub fn new(event: &'a str) -> Self {
        ScopedEvent { event }
    }
}

impl std::ops::Drop for ScopedEvent<'_> {
    fn drop(&mut self) {
        emit(self.event);
    }
}

lazy_static::lazy_static! {
    pub static ref TEST_HOOK_SERVER: TestHookServer = TestHookServer::new();
}

pub struct TestHookServer {
    sock_path: Mutex<Option<String>>,
    clients: Mutex<Vec<UnixStream>>,
    pending_pauses: Mutex<HashSet<String>>,
    pause_cv: Condvar,
}

impl TestHookServer {
    fn new() -> Self {
        TestHookServer {
            sock_path: Mutex::new(None),
            clients: Mutex::new(vec![]),
            pending_pauses: Mutex::new(HashSet::new()),
            pause_cv: Condvar::new(),
        }
    }

    pub fn set_socket_path(&self, path: String) {
        let mut sock_path = self.sock_path.lock();
        *sock_path = Some(path);
    }

    pub fn wait_for_connect(&self) -> anyhow::Result<()> {
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            {
                let clients = self.clients.lock();
                if !clients.is_empty() {
                    return Ok(());
                }
            }

            std::thread::sleep(sleep_dur);
            sleep_dur *= 2;
        }

        Err(anyhow!("no connection to test hook server"))
    }

    /// start is the background thread to listen on a unix socket
    /// for a test harness to dial in so it can wait for events.
    /// The caller is responsible for spawning the worker thread.
    /// Events are pushed to everyone who has dialed in as a
    /// newline delimited stream of event tags.
    pub fn start(&self) {
        let sock_path: String;
        {
            let sock_path_m = self.sock_path.lock();
            match &*sock_path_m {
                Some(s) => {
                    sock_path = String::from(s);
                }
                None => {
                    error!("you must call set_socket_path before calling start");
                    return;
                }
            };
        }

        let listener = match UnixListener::bind(&sock_path).context("binding to socket") {
            Ok(l) => l,
            Err(e) => {
                error!("error binding to test hook socket: {:?}", e);
                return;
            }
        };
        info!("listening for test hook connections on {}", &sock_path);
        for stream in listener.incoming() {
            info!("accepted new test hook client");
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    error!("error accepting connection to test hook server: {:?}", e);
                    continue;
                }
            };
            match stream.try_clone() {
                Ok(stream_clone) => {
                    let mut clients = self.clients.lock();
                    clients.push(stream);

                    thread::spawn(move || {
                        TEST_HOOK_SERVER.handle_client(stream_clone);
                    });
                }
                Err(e) => {
                    error!("error cloning test hook stream: {:?}", e);
                }
            }
        }
    }

    fn emit_event(&self, event: &str) {
        info!("emitting event '{}'", event);
        let event_line = format!("{event}\n");
        let clients = self.clients.lock();
        for mut client in clients.iter() {
            if let Err(e) = client.write_all(event_line.as_bytes()) {
                error!("error emitting '{}' event: {:?}", event, e);
            }
        }
    }

    fn handle_client(&self, stream: UnixStream) {
        let mut reader = std::io::BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let parts: Vec<&str> = line.trim().splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let cmd = parts[0];
                        let event = parts[1];
                        match cmd {
                            "pause-at" => {
                                info!("test requested pause at '{}'", event);
                                self.pending_pauses.lock().insert(event.to_string());
                            }
                            "release" => {
                                info!("test requested release of '{}'", event);
                                self.pending_pauses.lock().remove(event);
                                self.pause_cv.notify_all();
                            }
                            _ => error!("unknown test hook command: {}", cmd),
                        }
                    }
                }
                Err(e) => {
                    error!("error reading from test hook client: {:?}", e);
                    break;
                }
            }
        }
    }

    fn maybe_pause(&self, event: &str) {
        let mut pending = self.pending_pauses.lock();
        if pending.contains(event) {
            info!("pausing at '{}'", event);
            self.emit_event(&format!("paused-at {event}"));
            while pending.contains(event) {
                self.pause_cv.wait(&mut pending);
            }
            info!("resuming from '{}'", event);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn failure_points_from_environment() {
        // Use a subprocess to isolate the environment and process-wide state
        // from other tests, without changing the environment of running
        // threads.
        if std::env::var_os("SHPOOL_TEST__FAILURE_POINTS_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "test_hooks::tests::failure_points_from_environment"])
                .env("SHPOOL_TEST__FAILURE_POINTS_CHILD", "1")
                .env("SHPOOL_TEST__FAILURE_POINTS", "first, second,,first")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }

        assert!(!super::fail_once("unknown"));
        assert!(!super::fail_once(""));
        let workers: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (super::fail_once("first"), super::fail_once("second"))))
            .collect();
        let results: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
        let expected = usize::from(cfg!(feature = "test_hooks"));
        assert_eq!(results.iter().filter(|(first, _)| *first).count(), expected);
        assert_eq!(results.iter().filter(|(_, second)| *second).count(), expected);
        assert!(!super::fail_once("first"));
        assert!(!super::fail_once("second"));
    }
}
