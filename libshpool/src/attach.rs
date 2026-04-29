// Copyright 2023-2026 Google LLC
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

use std::{
    collections::HashMap,
    env, fmt, io,
    os::fd::AsFd,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread, time,
};

use anyhow::{anyhow, bail, Context};
use nix::unistd;
use shpool_protocol::{
    AttachHeader, AttachReplyHeader, ConnectHeader, DetachReply, DetachRequest, MaybeSwitch,
    ResizeReply, ResizeRequest, SessionMessageReply, SessionMessageRequest,
    SessionMessageRequestPayload, TtySize,
};
use tracing::{debug, error, info, warn};

use crate::{
    config, duration, protocol,
    protocol::{ClientResult, PipeBytesResult},
    template, test_hooks,
    tty::TtySizeExt as _,
};

const MAX_FORCE_RETRIES: usize = 20;

#[allow(clippy::too_many_arguments)]
pub fn run(
    config_manager: config::Manager,
    name: String,
    force: bool,
    background: bool,
    ttl: Option<String>,
    cmd: Option<String>,
    dir: Option<String>,
    socket: PathBuf,
) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING ATTACH ============================\n\n");
    test_hooks::emit("attach-startup");

    let session_name_tmpl = template::Template::new(&name).context("parsing session name tmpl")?;

    let ttl = match &ttl {
        Some(src) => match duration::parse(src.as_str()) {
            Ok(d) => Some(d),
            Err(e) => {
                bail!("could not parse ttl: {:?}", e);
            }
        },
        None => None,
    };

    let attach =
        Attach { config_manager, session_name_tmpl, force, background, ttl, cmd, dir, socket };

    attach.run()
}

struct Attach {
    config_manager: config::Manager,
    session_name_tmpl: template::Template,
    force: bool,
    background: bool,
    ttl: Option<time::Duration>,
    cmd: Option<String>,
    dir: Option<String>,
    socket: PathBuf,
}

impl Attach {
    fn run(self) -> anyhow::Result<()> {
        // This is the first time we dial the daemon, so we do want to show
        // warnings. After this we shouldn't show them again.
        let mut client = self.dial_client(false).context("dialing daemon")?;
        info!("dialed initial conn for GetVars");

        client.write_connect_header(ConnectHeader::GetVars).context("getting vars")?;
        let mut maybe_switch: MaybeSwitch = client.read_reply().context("reading reply")?;

        let var_map = maybe_switch.vars.iter().cloned().collect();
        let mut resolved_name = self.session_name_tmpl.apply(&var_map);

        let sig_handler_session_name_slot = if !self.background {
            Some(SignalHandler::new(resolved_name.clone(), self.socket.clone()).spawn()?)
        } else {
            None
        };

        info!("looping on attach_with_name");
        loop {
            match self.attach_with_name(resolved_name) {
                Ok(AttachResult::Done) => return Ok(()),
                Ok(AttachResult::Switch(s)) => maybe_switch = s,
                Err(e) => return Err(e),
            }

            let var_map = maybe_switch.vars.iter().cloned().collect();
            resolved_name = self.session_name_tmpl.apply(&var_map);

            if let Some(ref slot) = sig_handler_session_name_slot {
                let mut slot = slot.lock().unwrap();
                *slot = resolved_name.clone();
            }
        }
    }

    /// Attach with the given resolved name. This will run until exit or until
    /// we need to reconnect due to
    pub fn attach_with_name(&self, resolved_name: String) -> anyhow::Result<AttachResult> {
        if resolved_name.is_empty() {
            eprintln!("blank session names are not allowed");
            return Ok(AttachResult::Done);
        }
        if resolved_name.contains(char::is_whitespace) {
            eprintln!("session name '{}' may not have whitespace", resolved_name);
            return Ok(AttachResult::Done);
        }

        let mut detached = false;
        let mut tries = 0;
        let attach_client = loop {
            match self.dial_attach(resolved_name.as_str()) {
                Ok(client) => break client,
                Err(err) => match err.downcast() {
                    Ok(BusyError) if !self.force => {
                        eprintln!("session '{resolved_name}' already has a terminal attached");
                        return Ok(AttachResult::Done);
                    }
                    Ok(BusyError) => {
                        if !detached {
                            let mut client = self.dial_client(true)?;
                            client
                                .write_connect_header(ConnectHeader::Detach(DetachRequest {
                                    sessions: vec![resolved_name.clone()],
                                }))
                                .context("writing detach request header")?;
                            let detach_reply: DetachReply =
                                client.read_reply().context("reading reply")?;
                            if !detach_reply.not_found_sessions.is_empty() {
                                warn!("could not find session '{}' to detach it", resolved_name);
                            }

                            detached = true;
                        }
                        thread::sleep(time::Duration::from_millis(100));

                        if tries > MAX_FORCE_RETRIES {
                            eprintln!("session '{resolved_name}' already has a terminal which remains attached even after attempting to detach it");
                            return Err(anyhow!("could not detach session, forced attach failed"));
                        }
                        tries += 1;
                    }
                    Err(err) => return Err(err),
                },
            }
        };
        info!("got attach client");

        if self.background {
            // Close the attached connection first so the daemon can observe EOF.
            // We still send an explicit Detach on a fresh connection as a best-effort
            // fallback in case EOF processing is delayed.
            drop(attach_client);
            let mut client = self.dial_client(true)?;
            client
                .write_connect_header(ConnectHeader::Detach(DetachRequest {
                    sessions: vec![resolved_name.clone()],
                }))
                .context("writing detach request header")?;
            let detach_reply: DetachReply = client.read_reply().context("reading reply")?;
            if !detach_reply.not_found_sessions.is_empty() {
                warn!("could not find session '{}' to detach it", resolved_name);
            }
            if !detach_reply.not_attached_sessions.is_empty() {
                debug!(
                    "session '{}' was already detached while processing background detach request (expected)",
                    resolved_name
                );
            }
            return Ok(AttachResult::Done);
        }

        info!("entering bidi streaming mode");
        let session_name_tmpl = self.session_name_tmpl.clone();
        match attach_client.pipe_bytes(move |maybe_switch: &MaybeSwitch| {
            let var_map: HashMap<String, String> = maybe_switch.vars.iter().cloned().collect();
            session_name_tmpl.apply(&var_map) != resolved_name
        }) {
            Ok(PipeBytesResult::Exit(exit_status)) => std::process::exit(exit_status),
            Ok(PipeBytesResult::MaybeSwitch(s)) => Ok(AttachResult::Switch(s)),
            Err(e) => Err(e),
        }
    }

    /// Attach to a session and return the connected client without piping
    /// stdio.
    fn dial_attach(&self, name: &str) -> anyhow::Result<protocol::Client> {
        let mut client = self.dial_client(true)?;

        let tty_size = match TtySize::from_fd(0) {
            Ok(s) => s,
            Err(e) => {
                warn!("stdin is not a tty, using default size (err: {e:?})");
                TtySize { rows: 24, cols: 80, xpixel: 0, ypixel: 0 }
            }
        };

        let forward_env = self.config_manager.get().forward_env.clone();
        let mut local_env_keys = vec!["TERM", "DISPLAY", "LANG", "SSH_AUTH_SOCK"];
        if let Some(fenv) = &forward_env {
            for var in fenv.iter() {
                local_env_keys.push(var);
            }
        }
        info!("local env keys: {local_env_keys:?}");

        let cwd = String::from(env::current_dir().context("getting cwd")?.to_string_lossy());
        let default_dir =
            self.config_manager.get().default_dir.clone().unwrap_or(String::from("$HOME"));
        let start_dir = match (default_dir.as_str(), self.dir.as_deref()) {
            (".", None) => Some(cwd),
            ("$HOME", None) => None,
            (d, None) => Some(String::from(d)),
            (_, Some(".")) => Some(cwd),
            (_, Some(d)) => Some(String::from(d)),
        };

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
                ttl_secs: self.ttl.map(|d| d.as_secs()),
                cmd: self.cmd.clone(),
                dir: start_dir,
            }))
            .context("writing attach header")?;

        let attach_resp: AttachReplyHeader = client.read_reply().context("reading attach reply")?;
        info!("attach_resp.status={:?}", attach_resp.status);

        {
            use shpool_protocol::AttachStatus::*;
            match attach_resp.status {
                Busy => {
                    return Err(BusyError.into());
                }
                Forbidden(reason) => {
                    eprintln!("forbidden: {reason}");
                    return Err(anyhow!("forbidden: {reason}"));
                }
                Attached { warnings } => {
                    for warning in warnings.into_iter() {
                        eprintln!("shpool: warn: {warning}");
                    }
                    info!("attached to an existing session: '{}'", name);
                }
                Created { warnings } => {
                    for warning in warnings.into_iter() {
                        eprintln!("shpool: warn: {warning}");
                    }
                    info!("created a new session: '{}'", name);
                }
                UnexpectedError(err) => {
                    return Err(anyhow!("BUG: unexpected error attaching to '{}': {}", name, err));
                }
            }
        }

        Ok(client)
    }

    // Dial the daemon. If silent is true, don't attempt to warn the user.
    // After the first dial, silent should always be true.
    fn dial_client(&self, silent: bool) -> anyhow::Result<protocol::Client> {
        match protocol::Client::new(&self.socket) {
            Ok(ClientResult::JustClient(c)) => Ok(c),
            Ok(ClientResult::VersionMismatch { warning, client }) => {
                if silent {
                    return Ok(client);
                }

                if self.background {
                    eprintln!(
                        "warning: {warning}, proceeding in background mode; try restarting your daemon"
                    );
                } else {
                    eprintln!("warning: {warning}, try restarting your daemon");
                    eprintln!("hit enter to continue anyway or ^C to exit");

                    let mut buf = [0u8; 1];
                    loop {
                        match unistd::read(io::stdin().as_fd(), &mut buf) {
                            Ok(0) => break,
                            Ok(1) if buf[0] == b'\n' => break,
                            Ok(_) => continue,
                            Err(nix::errno::Errno::EINTR) => continue,
                            Err(e) => {
                                return Err(anyhow::Error::from(e))
                                    .context("waiting for a continue through a version mismatch")
                            }
                        }
                    }
                    info!("user continued through version mismatch");
                }

                Ok(client)
            }
            Err(err) => {
                let io_err = err.downcast::<io::Error>()?;
                if io_err.kind() == io::ErrorKind::NotFound {
                    eprintln!("could not connect to daemon");
                }
                Err(io_err).context("connecting to daemon")
            }
        }
    }
}

#[derive(Debug)]
struct BusyError;
impl fmt::Display for BusyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BusyError")
    }
}
impl std::error::Error for BusyError {}

enum AttachResult {
    Done,
    Switch(MaybeSwitch),
}

//
// Signal Handling
//

struct SignalHandler {
    session_name: Arc<Mutex<String>>,
    socket: PathBuf,
}

impl SignalHandler {
    fn new(session_name: String, socket: PathBuf) -> Self {
        SignalHandler { session_name: Arc::new(Mutex::new(session_name)), socket }
    }

    fn spawn(self) -> anyhow::Result<Arc<Mutex<String>>> {
        use signal_hook::{consts::*, iterator::*};

        let session_name_slot = Arc::clone(&self.session_name);

        let sigs = vec![SIGWINCH];
        let mut signals = Signals::new(sigs).context("creating signal iterator")?;

        thread::spawn(move || {
            for signal in &mut signals {
                let res = match signal {
                    SIGWINCH => self.handle_sigwinch(),
                    sig => {
                        error!("unknown signal: {}", sig);
                        panic!("unknown signal: {sig}");
                    }
                };
                if let Err(e) = res {
                    error!("signal handler error: {:?}", e);
                }
            }
        });

        Ok(session_name_slot)
    }

    fn handle_sigwinch(&self) -> anyhow::Result<()> {
        info!("handle_sigwinch: enter");
        let mut client = match protocol::Client::new(&self.socket)? {
            ClientResult::JustClient(c) => c,
            // At this point, we've already warned the user and they
            // chose to continue anyway, so we shouldn't bother them
            // again.
            ClientResult::VersionMismatch { client, .. } => client,
        };

        let tty_size = TtySize::from_fd(0).context("getting tty size")?;
        info!("handle_sigwinch: tty_size={:?}", tty_size);

        // write the request on a new, seperate connection
        client
            .write_connect_header(ConnectHeader::SessionMessage(SessionMessageRequest {
                session_name: self.get_session_name(),
                payload: SessionMessageRequestPayload::Resize(ResizeRequest {
                    tty_size: tty_size.clone(),
                }),
            }))
            .context("writing resize request")?;

        let reply: SessionMessageReply =
            client.read_reply().context("reading session message reply")?;
        match reply {
            SessionMessageReply::NotFound => {
                warn!(
                    "handle_sigwinch: sent resize for session '{}', but the daemon has no record of that session",
                    self.get_session_name()
                );
            }
            SessionMessageReply::Resize(ResizeReply::Ok) => {
                info!(
                    "handle_sigwinch: resized session '{}' to {:?}",
                    self.get_session_name(),
                    tty_size
                );
            }
            reply => {
                warn!("handle_sigwinch: unexpected resize reply: {:?}", reply);
            }
        }

        Ok(())
    }

    fn get_session_name(&self) -> String {
        let session_name = self.session_name.lock().unwrap();
        session_name.clone()
    }
}
