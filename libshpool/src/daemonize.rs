// Copyright 2024 Google LLC
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

use std::{ffi::OsStr, os::unix::net::UnixStream, path::Path, process, thread, time::Duration};

use crate::{config, consts, Args};

use anyhow::{anyhow, Context};
use tracing::info;

/// Check if we can connect to the control socket, and if we
/// can't, fork the daemon in the background.
pub fn maybe_fork_daemon<B, P>(
    config_manager: &config::Manager,
    args: &Args,
    shpool_bin: B,
    control_sock: P,
) -> anyhow::Result<()>
where
    B: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let control_sock = control_sock.as_ref();

    if UnixStream::connect(control_sock).is_ok() {
        info!("daemon already running on {:?}, no need to autodaemonize", control_sock);
        // There is already a daemon listening on the control socket, we
        // don't need to do anything.
        return Ok(());
    }
    info!("no daemon running on {:?}, autodaemonizing", control_sock);

    let log_file = control_sock.with_file_name("daemonized-shpool.log");

    let mut cmd = process::Command::new(shpool_bin);
    if let Some(config_file) = &args.config_file {
        cmd.arg("--config-file").arg(config_file);
    }
    cmd.arg("--log-file")
        .arg(log_file)
        .arg("--socket")
        .arg(control_sock.as_os_str())
        .arg("daemon")
        .env(consts::AUTODAEMONIZE_VAR, "true")
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .spawn()
        .context("launching background daemon")?;
    info!("launched background daemon");

    // Now poll with exponential backoff until we can dial the control socket.
    if config_manager.get().nodaemonize_timeout.unwrap_or(false) {
        info!("waiting for daemon to come up with no timeout");
        let mut sleep_ms = 10;
        let max_sleep_ms = 2000;
        loop {
            if UnixStream::connect(control_sock).is_ok() {
                info!("connected to freshly launched background daemon");
                return Ok(());
            }

            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms *= 2;
            if sleep_ms > max_sleep_ms {
                sleep_ms = max_sleep_ms;
            }
        }
    } else {
        info!("waiting for daemon to come up with timeout");
        // `sum(10*(2**x) for x in range(9))` = 5110 ms = ~5 s
        let mut sleep_ms = 10;
        for _ in 0..9 {
            if UnixStream::connect(control_sock).is_ok() {
                info!("connected to freshly launched background daemon");
                return Ok(());
            }

            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms *= 2;
        }
    }

    Err(anyhow!("daemonizing: launched daemon, but control socket never came up"))
}
