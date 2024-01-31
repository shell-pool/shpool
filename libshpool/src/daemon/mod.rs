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

use std::{os::unix::net::UnixListener, path::PathBuf};

use anyhow::Context;
use tracing::{info, instrument};

use super::config;

mod etc_environment;
mod exit_notify;
pub mod keybindings;
mod prompt;
mod server;
mod shell;
mod signals;
mod systemd;
mod ttl_reaper;

#[instrument(skip_all)]
pub fn run(
    config_file: Option<String>,
    runtime_dir: PathBuf,
    socket: PathBuf,
) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let config = config::read_config(&config_file)?;
    let server = server::Server::new(config, runtime_dir);

    let (cleanup_socket, listener) = match systemd::activation_socket() {
        Ok(l) => {
            info!("using systemd activation socket");
            (None, l)
        }
        Err(e) => {
            info!("no systemd activation socket: {:?}", e);
            (Some(socket.clone()), UnixListener::bind(&socket).context("binding to socket")?)
        }
    };
    // spawn the signal handler thread in the background
    signals::Handler::new(cleanup_socket.clone()).spawn()?;

    server::Server::serve(server, listener)?;

    if let Some(sock) = cleanup_socket {
        std::fs::remove_file(sock).context("cleaning up socket on exit")?;
    } else {
        info!("systemd manages the socket, so not cleaning it up");
    }

    Ok(())
}
