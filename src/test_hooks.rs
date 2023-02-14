// tooling gets confused by the conditional compilation
#![allow(dead_code)]

// The test_hooks module provides a mechanism for exposing events to
// the test harness so that it does not have to rely on buggy and slow
// sleeps in order to test various scenarios. The basic idea is that
// we publish a unix socket and then clients can listen for specific
// named events in order to block until they have occurred.
use std::{
    io::Write,
    os::unix::net::{
        UnixListener,
        UnixStream,
    },
    sync::Mutex,
    time,
};

use anyhow::{
    anyhow,
    Context,
};
use tracing::{
    error,
    info,
};

#[cfg(feature = "test_hooks")]
#[macro_export]
macro_rules! emit {
    ($e:expr) => {
        $crate::test_hooks::emit_event_impl($e);
    };
}

#[cfg(not(feature = "test_hooks"))]
#[macro_export]
macro_rules! emit {
    ($e:expr) => {}; // no-op
}

pub(crate) use emit;

#[cfg(feature = "test_hooks")]
#[macro_export]
macro_rules! scoped {
    ($name:ident, $e:expr) => {
        let $name = $crate::test_hooks::ScopedEvent::new($e);
    };
}

#[cfg(not(feature = "test_hooks"))]
#[macro_export]
macro_rules! scoped {
    ($name:ident, $e:expr) => {}; // no-op
}

pub(crate) use scoped;

/// ScopedEvent emits an event when it goes out of scope
pub struct ScopedEvent<'a> {
    event: &'a str,
}
impl<'a> ScopedEvent<'a> {
    pub fn new(event: &'a str) -> Self {
        ScopedEvent { event }
    }
}
impl<'a> std::ops::Drop for ScopedEvent<'a> {
    fn drop(&mut self) {
        emit_event_impl(self.event);
    }
}

pub fn emit_event_impl(event: &str) {
    let sock_path = TEST_HOOK_SERVER.sock_path.lock().unwrap();
    if sock_path.is_some() {
        TEST_HOOK_SERVER.emit_event(event);
    }
}

lazy_static::lazy_static! {
    pub static ref TEST_HOOK_SERVER: TestHookServer = TestHookServer::new();
}

pub struct TestHookServer {
    sock_path: Mutex<Option<String>>,
    clients: Mutex<Vec<UnixStream>>,
}

impl TestHookServer {
    fn new() -> Self {
        TestHookServer {
            sock_path: Mutex::new(None),
            clients: Mutex::new(vec![]),
        }
    }

    pub fn set_socket_path(&self, path: String) {
        let mut sock_path = self.sock_path.lock().unwrap();
        *sock_path = Some(path);
    }

    pub fn wait_for_connect(&self) -> anyhow::Result<()> {
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            {
                let clients = self.clients.lock().unwrap();
                if clients.len() > 0 {
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
            let sock_path_m = self.sock_path.lock().unwrap();
            match &*sock_path_m {
                Some(s) => {
                    sock_path = String::from(s);
                },
                None => {
                    error!("you must call set_socket_path before calling start");
                    return;
                },
            };
        }

        let listener = match UnixListener::bind(&sock_path).context("binding to socket") {
            Ok(l) => l,
            Err(e) => {
                error!("error binding to test hook socket: {:?}", e);
                return;
            },
        };
        info!("listening for test hook connections on {}", &sock_path);
        for stream in listener.incoming() {
            info!("accepted new test hook client");
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    error!("error accepting connection to test hook server: {:?}", e);
                    continue;
                },
            };
            let mut clients = self.clients.lock().unwrap();
            clients.push(stream);
        }
    }

    fn emit_event(&self, event: &str) {
        info!("emitting event '{}'", event);
        let event_line = format!("{}\n", event);
        let clients = self.clients.lock().unwrap();
        for mut client in clients.iter() {
            if let Err(e) = client.write_all(event_line.as_bytes()) {
                error!("error emitting '{}' event: {:?}", event, e);
            }
        }
    }
}
