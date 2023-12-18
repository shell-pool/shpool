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

use std::{env, fmt, io, path::PathBuf, thread, time};

use anyhow::{anyhow, bail, Context};
use tracing::{error, info, warn};

use super::{
    config, duration, protocol,
    protocol::{AttachHeader, ConnectHeader},
    test_hooks, tty,
};

const MAX_FORCE_RETRIES: usize = 20;

pub fn run(
    config_file: Option<String>,
    name: String,
    force: bool,
    ttl: Option<String>,
    cmd: Option<String>,
    socket: PathBuf,
) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING ATTACH ============================\n\n");
    test_hooks::emit("attach-startup");
    SignalHandler::new(name.clone(), socket.clone()).spawn()?;

    let config = config::read_config(&config_file)?;

    let ttl = match &ttl {
        Some(src) => match duration::parse(src.as_str()) {
            Ok(d) => Some(d),
            Err(e) => {
                bail!("could not parse ttl: {:?}", e);
            }
        },
        None => None,
    };

    let mut detached = false;
    let mut tries = 0;
    while let Err(err) = do_attach(&config, name.as_str(), &ttl, &cmd, &socket) {
        match err.downcast() {
            Ok(BusyError) if !force => {
                eprintln!("session '{}' already has a terminal attached", name);
                return Ok(());
            }
            Ok(BusyError) => {
                if !detached {
                    let mut client = dial_client(&socket)?;
                    client
                        .write_connect_header(ConnectHeader::Detach(protocol::DetachRequest {
                            sessions: vec![name.clone()],
                        }))
                        .context("writing detach request header")?;
                    let detach_reply: protocol::DetachReply =
                        client.read_reply().context("reading reply")?;
                    if detach_reply.not_found_sessions.len() > 0 {
                        warn!("could not find session '{}' to detach it", name);
                    }

                    detached = true;
                }
                thread::sleep(time::Duration::from_millis(100));

                if tries > MAX_FORCE_RETRIES {
                    eprintln!(
                        "session '{}' already has a terminal which remains attached even after attempting to detach it",
                        name
                    );
                    return Err(anyhow!("could not detach session, forced attach failed"));
                }
                tries += 1;
            }
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

#[derive(Debug)]
struct BusyError;
impl fmt::Display for BusyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BusyError")
    }
}
impl std::error::Error for BusyError {}

fn do_attach(
    config: &config::Config,
    name: &str,
    ttl: &Option<time::Duration>,
    cmd: &Option<String>,
    socket: &PathBuf,
) -> anyhow::Result<()> {
    let mut client = dial_client(&socket)?;

    let tty_size = match tty::Size::from_fd(0) {
        Ok(s) => s,
        Err(e) => {
            warn!("stdin is not a tty, using default size (err: {:?})", e);
            tty::Size { rows: 24, cols: 80 }
        }
    };

    let mut local_env_keys = vec!["TERM", "DISPLAY", "LANG", "SSH_AUTH_SOCK"];
    if let Some(forward_env) = &config.forward_env {
        for var in forward_env.iter() {
            local_env_keys.push(var);
        }
    }

    client
        .write_connect_header(ConnectHeader::Attach(AttachHeader {
            name: String::from(name),
            local_tty_size: tty_size,
            local_env: local_env_keys
                .into_iter()
                .filter_map(|var| {
                    let val = env::var(var).context("resolving var").ok()?;
                    Some((String::from(var), val))
                })
                .collect::<Vec<_>>(),
            ttl_secs: ttl.map(|d| d.as_secs()),
            cmd: cmd.clone(),
        }))
        .context("writing attach header")?;

    let attach_resp: protocol::AttachReplyHeader =
        client.read_reply().context("reading attach reply")?;
    info!("attach_resp.status={:?}", attach_resp.status);

    {
        use protocol::AttachStatus::*;
        match attach_resp.status {
            Busy => {
                return Err(BusyError.into());
            }
            Forbidden(reason) => {
                eprintln!("forbidden: {}", reason);
                return Err(anyhow!("forbidden: {}", reason));
            }
            Attached { warnings } => {
                for warning in warnings.into_iter() {
                    eprintln!("shpool: warn: {}", warning);
                }
                info!("attached to an existing session: '{}'", name);
            }
            Created { warnings } => {
                for warning in warnings.into_iter() {
                    eprintln!("shpool: warn: {}", warning);
                }
                info!("created a new session: '{}'", name);
            }
            UnexpectedError(err) => {
                return Err(anyhow!("BUG: unexpected error attaching to '{}': {}", name, err));
            }
        }
    }

    match client.pipe_bytes() {
        Ok(exit_status) => std::process::exit(exit_status),
        Err(e) => Err(e),
    }
}

fn dial_client(socket: &PathBuf) -> anyhow::Result<protocol::Client> {
    match protocol::Client::new(socket) {
        Ok(c) => Ok(c),
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                eprintln!("could not connect to daemon");
            }
            Err(io_err).context("connecting to daemon")
        }
    }
}

//
// Signal Handling
//

struct SignalHandler {
    session_name: String,
    socket: PathBuf,
}

impl SignalHandler {
    fn new(session_name: String, socket: PathBuf) -> Self {
        SignalHandler { session_name, socket }
    }

    fn spawn(self) -> anyhow::Result<()> {
        use signal_hook::{consts::*, iterator::*};

        let sigs = vec![SIGWINCH];
        let mut signals = Signals::new(&sigs).context("creating signal iterator")?;

        thread::spawn(move || {
            for signal in &mut signals {
                let res = match signal {
                    SIGWINCH => self.handle_sigwinch(),
                    sig => {
                        error!("unknown signal: {}", sig);
                        panic!("unknown signal: {}", sig);
                    }
                };
                if let Err(e) = res {
                    error!("signal handler error: {:?}", e);
                }
            }
        });

        Ok(())
    }

    fn handle_sigwinch(&self) -> anyhow::Result<()> {
        info!("handle_sigwinch: enter");
        let mut client = protocol::Client::new(&self.socket)?;

        let tty_size = tty::Size::from_fd(0).context("getting tty size")?;
        info!("handle_sigwinch: tty_size={:?}", tty_size);

        // write the request on a new, seperate connection
        client
            .write_connect_header(protocol::ConnectHeader::SessionMessage(
                protocol::SessionMessageRequest {
                    session_name: self.session_name.clone(),
                    payload: protocol::SessionMessageRequestPayload::Resize(
                        protocol::ResizeRequest { tty_size: tty_size.clone() },
                    ),
                },
            ))
            .context("writing resize request")?;

        let reply: protocol::SessionMessageReply =
            client.read_reply().context("reading session message reply")?;
        match reply {
            protocol::SessionMessageReply::NotFound => {
                warn!(
                    "handle_sigwinch: sent resize for session '{}', but the daemon has no record of that session",
                    self.session_name
                );
            }
            protocol::SessionMessageReply::Resize(protocol::ResizeReply::Ok) => {
                info!("handle_sigwinch: resized session '{}' to {:?}", self.session_name, tty_size);
            }
            reply => {
                warn!("handle_sigwinch: unexpected resize reply: {:?}", reply);
            }
        }

        Ok(())
    }
}
