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

use std::{env, os::unix::net::UnixListener, path::PathBuf};

use anyhow::Context;
use tracing::{info, instrument};

use crate::{config, consts, hooks};

mod etc_environment;
pub(crate) mod events;
mod exit_notify;
pub mod keybindings;
mod pager;
mod prompt;
mod server;
mod shell;
mod show_motd;
mod signals;
mod systemd;
mod trie;
mod ttl_reaper;

#[instrument(skip_all)]
pub fn run(
    config_manager: config::Manager,
    runtime_dir: PathBuf,
    hooks: Box<dyn hooks::Hooks + Send + Sync>,
    log_level_handle: tracing_subscriber::reload::Handle<
        tracing_subscriber::filter::LevelFilter,
        tracing_subscriber::registry::Registry,
    >,
    socket: PathBuf,
) -> anyhow::Result<()> {
    if let Ok(daemonize) = env::var(consts::AUTODAEMONIZE_VAR) {
        if daemonize == "true" {
            // Safety: this is executing before we have forked any threads,
            // so we are still in single-threaded mode, therefore it is safe
            // to mutate the global env.
            unsafe {
                env::remove_var(consts::AUTODAEMONIZE_VAR); // avoid looping
            }

            let pid_file = socket.with_file_name("daemonized-shpool.pid");

            info!("daemonizing with pid_file={:?}", pid_file);
            daemonize::Daemonize::new().pid_file(pid_file).start().context("daemonizing")?;
        }
    }

    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let events_socket = events::socket_path(&socket);
    let (events_bus, _events_handle) =
        events::EventBus::start(events_socket.clone()).context("starting events bus")?;
    let server =
        server::Server::new(config_manager, hooks, runtime_dir, log_level_handle, events_bus)?;

    let (cleanup_socket, listener) = match systemd::activation_socket() {
        Ok(l) => {
            info!("using systemd activation socket");
            (None, l)
        }
        Err(e) => {
            info!("no systemd activation socket: {:?}", e);
            // If a stale socket file exists (file on disk, nothing listening),
            // remove it before binding so we don't get EADDRINUSE.
            if let Err(connect_err) = std::os::unix::net::UnixStream::connect(&socket) {
                if connect_err.kind() == std::io::ErrorKind::ConnectionRefused {
                    info!("removing stale socket file at {:?}", socket);
                    std::fs::remove_file(&socket).context("removing stale socket before bind")?;
                }
            }
            (Some(socket.clone()), UnixListener::bind(&socket).context("binding to socket")?)
        }
    };

    // spawn the signal handler thread in the background. Both sockets need
    // explicit cleanup on signal exit because the process exits before any
    // RAII guard can run -- the signal path bypasses the sink's RAII socket
    // cleanup, so it stays as a belt-and-suspenders unlink.
    let mut socks_to_clean: Vec<PathBuf> = cleanup_socket.iter().cloned().collect();
    socks_to_clean.push(events_socket.clone());
    signals::Handler::new(socks_to_clean).spawn().context("spawning signal handler")?;

    server::Server::serve(server, listener)?;

    if let Some(sock) = cleanup_socket {
        std::fs::remove_file(sock).context("cleaning up socket on exit")?;
    } else {
        info!("systemd manages the socket, so not cleaning it up");
    }

    Ok(())
}
