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

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs, io, net,
    ops::Add,
    os,
    os::unix::{
        fs::PermissionsExt as _,
        net::{UnixListener, UnixStream},
        process::CommandExt as _,
    },
    path::{Path, PathBuf},
    process,
    sync::{Arc, Mutex},
    thread, time,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use nix::unistd;
use shpool_protocol::{
    AttachHeader, AttachReplyHeader, AttachStatus, ConnectHeader, DetachReply, DetachRequest,
    KillReply, KillRequest, ListReply, LogLevel, ResizeReply, Session, SessionMessageDetachReply,
    SessionMessageReply, SessionMessageRequest, SessionMessageRequestPayload, SessionStatus,
    SetLogLevelReply, SetLogLevelRequest, VersionHeader,
};
use tracing::{debug, error, info, instrument, span, warn, Level};

use crate::{
    config,
    config::MotdDisplayMode,
    consts,
    daemon::{
        etc_environment, exit_notify::ExitNotifier, hooks, pager, pager::PagerError, prompt, shell,
        show_motd, ttl_reaper,
    },
    protocol, test_hooks, tty, user,
};

const DEFAULT_INITIAL_SHELL_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";
const DEFAULT_OUTPUT_SPOOL_LINES: usize = 500;
const DEFAULT_PROMPT_PREFIX: &str = "shpool:$SHPOOL_SESSION_NAME ";

// Half a second should be more than enough time to handle any resize or
// or detach. If things are taking longer, we can't afford to keep waiting
// for the shell->client thread since session message calls are made with the
// global session table lock held.
const SESSION_MSG_TIMEOUT: time::Duration = time::Duration::from_millis(500);

pub struct Server {
    config: config::Manager,
    /// A map from shell session names to session descriptors.
    /// We wrap this in Arc<Mutex<_>> so that we can get at the
    /// table from different threads such as the SIGWINCH thread
    /// that is spawned during the attach process, and so that
    /// handle_conn can delegate to worker threads and quickly allow
    /// the main thread to become available to accept new connections.
    shells: Arc<Mutex<HashMap<String, Box<shell::Session>>>>,
    runtime_dir: PathBuf,
    register_new_reapable_session: crossbeam_channel::Sender<(String, Instant)>,
    hooks: Box<dyn hooks::Hooks + Send + Sync>,
    daily_messenger: Arc<show_motd::DailyMessenger>,
    log_level_handle: tracing_subscriber::reload::Handle<
        tracing_subscriber::filter::LevelFilter,
        tracing_subscriber::registry::Registry,
    >,
}

impl Server {
    #[instrument(skip_all)]
    pub fn new(
        config: config::Manager,
        hooks: Box<dyn hooks::Hooks + Send + Sync>,
        runtime_dir: PathBuf,
        log_level_handle: tracing_subscriber::reload::Handle<
            tracing_subscriber::filter::LevelFilter,
            tracing_subscriber::registry::Registry,
        >,
    ) -> anyhow::Result<Arc<Self>> {
        let shells = Arc::new(Mutex::new(HashMap::new()));
        // buffered so that we are unlikely to block when setting up a
        // new session
        let (new_sess_tx, new_sess_rx) = crossbeam_channel::bounded(10);
        let shells_tab = Arc::clone(&shells);
        thread::spawn(move || {
            if let Err(e) = ttl_reaper::run(new_sess_rx, shells_tab) {
                warn!("ttl reaper exited with error: {:?}", e);
            }
        });

        let daily_messenger = Arc::new(show_motd::DailyMessenger::new(config.clone())?);
        Ok(Arc::new(Server {
            config,
            shells,
            runtime_dir,
            register_new_reapable_session: new_sess_tx,
            hooks,
            daily_messenger,
            log_level_handle,
        }))
    }

    #[instrument(skip_all)]
    pub fn serve(server: Arc<Self>, listener: UnixListener) -> anyhow::Result<()> {
        test_hooks::emit("daemon-about-to-listen");
        let mut conn_counter = 0;
        for stream in listener.incoming() {
            info!("socket got a new connection");
            match stream {
                Ok(stream) => {
                    conn_counter += 1;
                    let conn_id = conn_counter;
                    let server = Arc::clone(&server);
                    thread::spawn(move || {
                        if let Err(err) = server.handle_conn(stream, conn_id) {
                            error!("handling new connection: {:?}", err)
                        }
                    });
                }
                Err(err) => {
                    error!("accepting stream: {:?}", err);
                }
            }
        }

        Ok(())
    }

    #[instrument(skip_all, fields(cid = conn_id))]
    fn handle_conn(&self, mut stream: UnixStream, conn_id: usize) -> anyhow::Result<()> {
        // We want to avoid timing out while blocking the main thread.
        // On macOS, set_read_timeout returns EINVAL if the peer has already
        // closed (e.g., a daemon presence probe). This is documented in the
        // macOS setsockopt(2) man page. Treat this the same as a broken pipe.
        if let Err(e) = stream.set_read_timeout(Some(consts::SOCK_STREAM_TIMEOUT)) {
            #[cfg(target_os = "macos")]
            if e.raw_os_error() == Some(libc::EINVAL) {
                info!("EINVAL setting read timeout, peer already closed (presence probe)");
                return Ok(());
            }
            return Err(e).context("setting read timeout on inbound session");
        }

        // advertize our protocol version to the client so that it can
        // warn about mismatches
        match protocol::encode_to(
            &VersionHeader {
                // We allow fake version to be injected for ease of testing.
                // Otherwise we would have to resort to some heinous build
                // contortions.
                version: match env::var("SHPOOL_TEST__OVERRIDE_VERSION") {
                    Ok(fake_version) => fake_version,
                    Err(_) => String::from(shpool_protocol::VERSION),
                },
            },
            &mut stream,
        ) {
            Ok(_) => {}
            Err(e)
                if e.root_cause()
                    .downcast_ref::<io::Error>()
                    .map(|ioe| ioe.kind() == io::ErrorKind::BrokenPipe)
                    .unwrap_or(false) =>
            {
                info!("broken pipe while writing version, likely just a daemon presence probe");
                return Ok(());
            }
            Err(e) => return Err(e).context("while writing version"),
        }

        let header = parse_connect_header(&mut stream).context("parsing connect header")?;

        if let Err(err) = check_peer(&stream) {
            if let ConnectHeader::Attach(_) = header {
                write_reply(
                    &mut stream,
                    AttachReplyHeader { status: AttachStatus::Forbidden(format!("{err:?}")) },
                )?;
            }
            stream.shutdown(net::Shutdown::Both).context("closing stream")?;
            return Err(err);
        };

        // Unset the read timeout before we pass things off to a
        // worker thread because it is perfectly fine for there to
        // be no new data for long periods of time when the users
        // is connected to a shell session.
        stream.set_read_timeout(None).context("unsetting read timout on inbound session")?;

        match header {
            ConnectHeader::Attach(h) => self.handle_attach(stream, conn_id, h),
            ConnectHeader::Detach(r) => self.handle_detach(stream, r),
            ConnectHeader::Kill(r) => self.handle_kill(stream, r),
            ConnectHeader::List => self.handle_list(stream),
            ConnectHeader::SessionMessage(header) => self.handle_session_message(stream, header),
            ConnectHeader::SetLogLevel(r) => self.handle_set_log_level(stream, r),
        }
    }

    #[instrument(skip_all)]
    fn handle_attach(
        &self,
        stream: UnixStream,
        conn_id: usize,
        header: AttachHeader,
    ) -> anyhow::Result<()> {
        let user_info = user::info().context("resolving user info")?;
        let shell_env = self.build_shell_env(&user_info, &header).context("building shell env")?;

        let (child_exit_notifier, inner_to_stream, pager_ctl_slot, status) =
            match self.select_shell_desc(stream, conn_id, &header, &user_info, &shell_env) {
                Ok(t) => t,
                Err(err)
                    if err
                        .downcast_ref::<ShellSelectionError>()
                        .map(|e| e == &ShellSelectionError::BusyShellSession)
                        .unwrap_or(false) =>
                {
                    return Ok(());
                }
                Err(err) => return Err(err)?,
            };
        info!("released lock on shells table");

        self.link_ssh_auth_sock(&header).context("linking SSH_AUTH_SOCK")?;
        self.populate_session_env_file(&header).context("populating session env file")?;

        if let (Some(child_exit_notifier), Some(inner), Some(pager_ctl_slot)) =
            (child_exit_notifier, inner_to_stream, pager_ctl_slot)
        {
            let mut child_done = false;
            let mut inner = inner.lock().unwrap();
            let client_stream = match inner.client_stream.as_mut() {
                Some(s) => s,
                None => {
                    return Err(anyhow!("no client stream, should be impossible"));
                }
            };

            let reply_status =
                write_reply(client_stream, AttachReplyHeader { status: status.clone() });
            if let Err(e) = reply_status {
                error!("error writing reply status: {:?}", e);
            }

            // If in pager motd mode, launch the pager and block until it is
            // done, picking up any tty size change that happened while the
            // user was examining the motd.
            let motd_mode = self.config.get().motd.clone().unwrap_or_default();
            let init_tty_size = if matches!(motd_mode, MotdDisplayMode::Pager { .. }) {
                match self.daily_messenger.display_in_pager(
                    client_stream,
                    pager_ctl_slot,
                    header.local_tty_size.clone(),
                    &shell_env,
                ) {
                    Ok(Some(new_size)) => {
                        info!("motd pager finished, reporting new tty size: {:?}", new_size);
                        new_size
                    }
                    Ok(None) => {
                        info!("not time to show the motd in the pager yet");
                        header.local_tty_size.clone()
                    }
                    Err(e) => match e.downcast::<PagerError>() {
                        Ok(PagerError::ClientHangup) => {
                            info!("client hung up while talking to pager, bailing");
                            return Ok(());
                        }
                        Err(e) => {
                            return Err(e).context("showing motd in pager")?;
                        }
                    },
                }
            } else {
                header.local_tty_size.clone()
            };

            info!("starting bidi stream loop");
            match inner.bidi_stream(conn_id, init_tty_size, child_exit_notifier) {
                Ok(done) => {
                    child_done = done;
                }
                Err(e) => {
                    error!("error shuffling bytes: {:?}", e);
                }
            }
            info!("bidi stream loop finished child_done={}", child_done);

            if child_done {
                info!("'{}' exited, removing from session table", header.name);
                if let Err(err) = self.hooks.on_shell_disconnect(&header.name) {
                    warn!("shell_disconnect hook: {:?}", err);
                }
                let _s = span!(Level::INFO, "2_lock(shells)").entered();
                let mut shells = self.shells.lock().unwrap();
                shells.remove(&header.name);

                // The child shell has exited, so the shell->client thread should
                // attempt to read from its stdout and get an error, causing
                // it to exit. That means we should be safe to join. We use
                // a separate if statement to avoid holding the shells lock
                // while we join the old thread.
                if let Some(h) = inner.shell_to_client_join_h.take() {
                    h.join()
                        .map_err(|e| anyhow!("joining shell->client after child exit: {:?}", e))?
                        .context("within shell->client thread after child exit")?;
                }
            } else {
                // Client disconnected but shell is still running - set last_disconnected_at
                {
                    let _s = span!(Level::INFO, "disconnect_lock(shells)").entered();
                    let shells = self.shells.lock().unwrap();
                    if let Some(session) = shells.get(&header.name) {
                        session.lifecycle_timestamps.lock().unwrap().last_disconnected_at =
                            Some(time::SystemTime::now());
                    }
                }
                if let Err(err) = self.hooks.on_client_disconnect(&header.name) {
                    warn!("client_disconnect hook: {:?}", err);
                }
            }

            info!("finished attach streaming section");
        } else {
            error!("internal error: failed to fetch just inserted session");
        }

        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn select_shell_desc(
        &self,
        mut stream: UnixStream,
        conn_id: usize,
        header: &AttachHeader,
        user_info: &user::Info,
        shell_env: &[(OsString, OsString)],
    ) -> anyhow::Result<(
        Option<Arc<ExitNotifier>>,
        Option<Arc<Mutex<shell::SessionInner>>>,
        Option<Arc<Mutex<Option<pager::PagerCtl>>>>,
        AttachStatus,
    )> {
        let warnings = vec![];

        // we unwrap to propagate the poison as an unwind
        let _s = span!(Level::INFO, "1_lock(shells)").entered();
        let mut shells = self.shells.lock().unwrap();

        let mut status = AttachStatus::Attached { warnings: warnings.clone() };
        if let Some(session) = shells.get(&header.name) {
            info!("found entry for '{}'", header.name);
            if let Ok(mut inner) = session.inner.try_lock() {
                let _s =
                    span!(Level::INFO, "aquired_lock(session.inner)", s = header.name).entered();
                // We have an existing session in our table, but the subshell
                // proc might have exited in the meantime, for example if the
                // user typed `exit` right before the connection dropped there
                // could be a zombie entry in our session table. We need to
                // re-check whether the subshell has exited before taking this over.
                //
                // N.B. this is still technically a race, but in practice it does
                // not ever cause problems, and there is no real way to avoid some
                // sort of race without just always creating a new session when
                // a shell exits, which would break `exit` typed at the shell prompt.
                match session.child_exit_notifier.wait(Some(time::Duration::from_millis(0))) {
                    None => {
                        // the channel is still open so the subshell is still running
                        info!("taking over existing session inner");
                        inner.client_stream = Some(stream.try_clone()?);
                        session.lifecycle_timestamps.lock().unwrap().last_connected_at =
                            Some(time::SystemTime::now());

                        if inner
                            .shell_to_client_join_h
                            .as_ref()
                            .map(|h| h.is_finished())
                            .unwrap_or(false)
                        {
                            warn!(
                                "child_exited chan unclosed, but shell->client thread has exited, clobbering with new subshell"
                            );
                            status = AttachStatus::Created { warnings };
                        }

                        // status is already attached
                    }
                    Some(exit_status) => {
                        // the channel is closed so we know the subshell exited
                        info!(
                            "stale inner, (child exited with status {}) clobbering with new subshell",
                            exit_status
                        );
                        status = AttachStatus::Created { warnings };
                    }
                }

                if inner.shell_to_client_join_h.as_ref().map(|h| h.is_finished()).unwrap_or(false) {
                    info!("shell->client thread finished, joining");
                    if let Some(h) = inner.shell_to_client_join_h.take() {
                        h.join()
                            .map_err(|e| anyhow!("joining shell->client on reattach: {:?}", e))?
                            .context("within shell->client thread on reattach")?;
                    }
                    assert!(matches!(status, AttachStatus::Created { .. }));
                }

                // fallthrough to bidi streaming
            } else {
                info!("busy shell session, doing nothing");
                // The stream is busy, so we just inform the client and close the stream.
                write_reply(&mut stream, AttachReplyHeader { status: AttachStatus::Busy })?;
                stream.shutdown(net::Shutdown::Both).context("closing stream")?;
                if let Err(err) = self.hooks.on_busy(&header.name) {
                    warn!("busy hook: {:?}", err);
                }
                return Err(ShellSelectionError::BusyShellSession)?;
            }
        } else {
            info!("no existing '{}' session, creating new one", &header.name);
            status = AttachStatus::Created { warnings };
        }

        if matches!(status, AttachStatus::Created { .. }) {
            info!("creating new subshell");
            if let Err(err) = self.hooks.on_new_session(&header.name) {
                warn!("new_session hook: {:?}", err);
            }
            let motd = self.config.get().motd.clone().unwrap_or_default();
            let session = self.spawn_subshell(
                conn_id,
                stream,
                header,
                user_info,
                shell_env,
                matches!(motd, MotdDisplayMode::Dump),
            )?;

            session.lifecycle_timestamps.lock().unwrap().last_connected_at =
                Some(time::SystemTime::now());
            shells.insert(header.name.clone(), Box::new(session));
            // fallthrough to bidi streaming
        } else if let Err(err) = self.hooks.on_reattach(&header.name) {
            warn!("reattach hook: {:?}", err);
        }

        // return a reference to the inner session so that
        // we can work with it without the global session
        // table lock held
        if let Some(session) = shells.get(&header.name) {
            Ok((
                Some(Arc::clone(&session.child_exit_notifier)),
                Some(Arc::clone(&session.inner)),
                Some(Arc::clone(&session.pager_ctl)),
                status,
            ))
        } else {
            Ok((None, None, None, status))
        }
    }

    #[instrument(skip_all)]
    fn link_ssh_auth_sock(&self, header: &AttachHeader) -> anyhow::Result<()> {
        if self.config.get().nosymlink_ssh_auth_sock.unwrap_or(false) {
            return Ok(());
        }

        if let Some(ssh_auth_sock) = header.local_env_get("SSH_AUTH_SOCK") {
            let symlink = self.ssh_auth_sock_symlink(PathBuf::from(&header.name));
            fs::create_dir_all(symlink.parent().ok_or(anyhow!("no symlink parent dir"))?)
                .context("could not create directory for SSH_AUTH_SOCK symlink")?;

            let sessions_dir =
                symlink.parent().and_then(|d| d.parent()).ok_or(anyhow!("no sessions dir"))?;
            let sessions_meta = fs::metadata(sessions_dir).context("stating sessions dir")?;

            // set RWX bits for user and no one else
            let mut sessions_perm = sessions_meta.permissions();
            if sessions_perm.mode() != 0o700 {
                sessions_perm.set_mode(0o700);
                fs::set_permissions(sessions_dir, sessions_perm)
                    .context("locking down permissions for sessions dir")?;
            }

            let _ = fs::remove_file(&symlink); // clean up the link if it exists already
            os::unix::fs::symlink(ssh_auth_sock, &symlink).context(format!(
                "could not symlink '{symlink:?}' to point to '{ssh_auth_sock:?}'"
            ))?;
        } else {
            info!("no SSH_AUTH_SOCK in client env, leaving it unlinked");
        }

        Ok(())
    }

    #[instrument(skip_all)]
    fn populate_session_env_file(&self, header: &AttachHeader) -> anyhow::Result<()> {
        let session_name = PathBuf::from(&header.name);
        fs::create_dir_all(self.session_dir(session_name.clone()))
            .context("creating session dir")?;

        let session_env_file = self.session_env_file(session_name);
        info!("populating {:?}", session_env_file);
        fs::write(
            session_env_file,
            header.local_env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n"),
        )
        .context("writing session env")?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_detach(&self, mut stream: UnixStream, request: DetachRequest) -> anyhow::Result<()> {
        let mut not_found_sessions = vec![];
        let mut not_attached_sessions = vec![];
        {
            let _s = span!(Level::INFO, "lock(shells)").entered();
            let shells = self.shells.lock().unwrap();
            for session in request.sessions.into_iter() {
                if let Some(s) = shells.get(&session) {
                    let _s = span!(Level::INFO, "lock(shell_to_client_ctl)", s = session).entered();
                    let shell_to_client_ctl = s.shell_to_client_ctl.lock().unwrap();
                    shell_to_client_ctl
                        .client_connection
                        .send(shell::ClientConnectionMsg::Disconnect)
                        .context("sending client detach to shell->client")?;
                    let status = shell_to_client_ctl
                        .client_connection_ack
                        .recv()
                        .context("getting client conn ack")?;
                    info!("detached session({}), status = {:?}", session, status);
                    if let shell::ClientConnectionStatus::DetachNone = status {
                        not_attached_sessions.push(session);
                    } else {
                        s.lifecycle_timestamps.lock().unwrap().last_disconnected_at =
                            Some(time::SystemTime::now());
                    }
                } else {
                    not_found_sessions.push(session);
                }
            }
        }

        write_reply(&mut stream, DetachReply { not_found_sessions, not_attached_sessions })
            .context("writing detach reply")?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_set_log_level(
        &self,
        mut stream: UnixStream,
        request: SetLogLevelRequest,
    ) -> anyhow::Result<()> {
        let level_filter = match request.level {
            LogLevel::Off => tracing_subscriber::filter::LevelFilter::OFF,
            LogLevel::Error => tracing_subscriber::filter::LevelFilter::ERROR,
            LogLevel::Warn => tracing_subscriber::filter::LevelFilter::WARN,
            LogLevel::Info => tracing_subscriber::filter::LevelFilter::INFO,
            LogLevel::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
            LogLevel::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
        };
        if let Err(e) = self.log_level_handle.modify(|filter| *filter = level_filter) {
            error!("modifying log level: {}", e);
        }

        write_reply(&mut stream, SetLogLevelReply {}).context("writing set log level reply")?;
        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_kill(&self, mut stream: UnixStream, request: KillRequest) -> anyhow::Result<()> {
        let mut not_found_sessions = vec![];
        {
            let _s = span!(Level::INFO, "lock(shells)").entered();
            let mut shells = self.shells.lock().unwrap();

            let mut to_remove = Vec::with_capacity(request.sessions.len());
            for session in request.sessions.into_iter() {
                if let Some(s) = shells.get(&session) {
                    s.kill().context("killing shell proc")?;

                    // we don't need to wait since the dedicated reaping thread is active
                    // even when a tty is not attached
                    to_remove.push(session);
                } else {
                    not_found_sessions.push(session);
                }
            }

            for session in to_remove.iter() {
                shells.remove(session);
            }
            if !to_remove.is_empty() {
                test_hooks::emit("daemon-handle-kill-removed-shells");
            }
        }

        write_reply(&mut stream, KillReply { not_found_sessions }).context("writing kill reply")?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_list(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        let _s = span!(Level::INFO, "lock(shells)").entered();
        let shells = self.shells.lock().unwrap();

        let sessions: anyhow::Result<Vec<Session>> = shells
            .iter()
            .map(|(k, v)| {
                let status = match v.inner.try_lock() {
                    Ok(_) => SessionStatus::Disconnected,
                    Err(_) => SessionStatus::Attached,
                };

                let timestamps = v.lifecycle_timestamps.lock().unwrap();
                let last_connected_at_unix_ms = timestamps
                    .last_connected_at
                    .map(|t| t.duration_since(time::UNIX_EPOCH).map(|d| d.as_millis() as i64))
                    .transpose()?;

                let last_disconnected_at_unix_ms = timestamps
                    .last_disconnected_at
                    .map(|t| t.duration_since(time::UNIX_EPOCH).map(|d| d.as_millis() as i64))
                    .transpose()?;

                Ok(Session {
                    name: k.to_string(),
                    started_at_unix_ms: v.started_at.duration_since(time::UNIX_EPOCH)?.as_millis()
                        as i64,
                    last_connected_at_unix_ms,
                    last_disconnected_at_unix_ms,
                    status,
                })
            })
            .collect();
        let sessions = sessions.context("collecting running session metadata")?;

        write_reply(&mut stream, ListReply { sessions })?;

        Ok(())
    }

    #[instrument(skip_all, fields(s = &header.session_name))]
    fn handle_session_message(
        &self,
        mut stream: UnixStream,
        header: SessionMessageRequest,
    ) -> anyhow::Result<()> {
        // create a slot to store our reply so we can do
        // our IO without the lock held.
        let reply = {
            let _s = span!(Level::INFO, "lock(shells)").entered();
            let shells = self.shells.lock().unwrap();
            if let Some(session) = shells.get(&header.session_name) {
                match header.payload {
                    SessionMessageRequestPayload::Resize(resize_request) => {
                        let _s = span!(Level::INFO, "lock(pager_ctl)").entered();
                        let pager_ctl = session.pager_ctl.lock().unwrap();
                        if let Some(pager_ctl) = pager_ctl.as_ref() {
                            info!("resizing pager");
                            pager_ctl
                                .tty_size_change
                                .send_timeout(resize_request.tty_size.clone(), SESSION_MSG_TIMEOUT)
                                .context("sending tty size change to pager")?;
                            pager_ctl
                                .tty_size_change_ack
                                .recv_timeout(SESSION_MSG_TIMEOUT)
                                .context("recving tty size change ack from pager")?;
                        } else {
                            let _s =
                                span!(Level::INFO, "resize_lock(shell_to_client_ctl)").entered();
                            let shell_to_client_ctl = session.shell_to_client_ctl.lock().unwrap();
                            shell_to_client_ctl
                                .tty_size_change
                                .send_timeout(resize_request.tty_size, SESSION_MSG_TIMEOUT)
                                .context("sending tty size change to shell->client")?;
                            shell_to_client_ctl
                                .tty_size_change_ack
                                .recv_timeout(SESSION_MSG_TIMEOUT)
                                .context("recving tty size ack")?;
                        }

                        SessionMessageReply::Resize(ResizeReply::Ok)
                    }
                    SessionMessageRequestPayload::Detach => {
                        let _s = span!(Level::INFO, "detach_lock(shell_to_client_ctl)").entered();
                        let shell_to_client_ctl = session.shell_to_client_ctl.lock().unwrap();
                        shell_to_client_ctl
                            .client_connection
                            .send_timeout(
                                shell::ClientConnectionMsg::Disconnect,
                                SESSION_MSG_TIMEOUT,
                            )
                            .context("sending client detach to shell->client")?;
                        let status = shell_to_client_ctl
                            .client_connection_ack
                            .recv_timeout(SESSION_MSG_TIMEOUT)
                            .context("getting client conn ack")?;
                        info!("detached session({}), status = {:?}", header.session_name, status);
                        SessionMessageReply::Detach(SessionMessageDetachReply::Ok)
                    }
                }
            } else {
                SessionMessageReply::NotFound
            }
        };

        write_reply(&mut stream, reply).context("handle_session_message: writing reply")?;

        Ok(())
    }

    /// Spawn a subshell and return the sessession descriptor for it. The
    /// session is wrapped in an Arc so the inner session can hold a Weak
    /// back-reference to the session.
    #[instrument(skip_all)]
    fn spawn_subshell(
        &self,
        conn_id: usize,
        client_stream: UnixStream,
        header: &AttachHeader,
        user_info: &user::Info,
        shell_env: &[(OsString, OsString)],
        dump_motd_on_new_session: bool,
    ) -> anyhow::Result<shell::Session> {
        let shell = if let Some(s) = &self.config.get().shell {
            s.clone()
        } else {
            user_info.default_shell.clone()
        };
        info!("user_info={:?}", user_info);

        // Build up the command we will exec while allocation is still chill.
        // We will exec this command after a fork, so we want to just inherit
        // stdout/stderr/stdin. The pty crate automatically `dup2`s the file
        // descriptors for us.
        let mut cmd = if let Some(cmd_str) = &header.cmd {
            let cmd_parts = shell_words::split(cmd_str).context("parsing cmd")?;
            info!("running cmd: {:?}", cmd_parts);
            if cmd_parts.is_empty() {
                return Err(anyhow!("no command to run"));
            }
            let mut cmd = process::Command::new(&cmd_parts[0]);
            cmd.args(&cmd_parts[1..]);
            cmd
        } else {
            let mut cmd = process::Command::new(&shell);
            if self.config.get().norc.unwrap_or(false) {
                if shell.ends_with("bash") {
                    cmd.arg("--norc").arg("--noprofile");
                } else if shell.ends_with("zsh") {
                    cmd.arg("--no-rcs");
                } else if shell.ends_with("fish") {
                    cmd.arg("--no-config");
                }
            }
            cmd
        };

        let start_dir = match header.dir.as_deref() {
            None => user_info.home_dir.clone(),
            Some(path) => String::from(path),
        };
        info!("spawning shell in '{}'", start_dir);
        cmd.current_dir(start_dir)
            .stdin(process::Stdio::inherit())
            .stdout(process::Stdio::inherit())
            .stderr(process::Stdio::inherit())
            // The env should mostly be set up by the shell sourcing
            // rc files and whatnot, so we will start things off with
            // an environment that is blank except for a few vars we inject
            // to avoid breakage and vars the user has asked us to inject.
            .env_clear();

        let term = shell_env.iter().filter(|(k, _)| k == "TERM").map(|(_, v)| v).next();
        cmd.envs(shell_env.to_vec());
        let fallback_terminfo = || match termini::TermInfo::from_name("xterm") {
            Ok(db) => Ok(db),
            Err(err) => {
                warn!("could not get xterm terminfo: {:?}", err);
                let empty_db = io::Cursor::new(vec![]);
                termini::TermInfo::parse(empty_db).context("getting terminfo db")
            }
        };
        let term_db = Arc::new(if let Some(term) = &term {
            match termini::TermInfo::from_name(term.to_string_lossy().as_ref())
                .context("resolving terminfo")
            {
                Ok(ti) => ti,
                Err(err) => {
                    warn!("could not get terminfo for '{:?}': {:?}", term, err);
                    fallback_terminfo()?
                }
            }
        } else {
            warn!("no $TERM, using default terminfo");
            match termini::TermInfo::from_env() {
                Ok(db) => db,
                Err(err) => {
                    warn!("could not get terminfo from env: {:?}", err);
                    fallback_terminfo()?
                }
            }
        });

        if header.cmd.is_none() {
            // spawn the shell as a login shell by setting
            // arg0 to be the basename of the shell path
            // proceeded with a "-". You can see sshd doing the
            // same thing if you look in the session.c file of
            // openssh.
            let shell_basename = Path::new(&shell)
                .file_name()
                .ok_or(anyhow!("error building login shell indicator"))?
                .to_str()
                .ok_or(anyhow!("error parsing shell name as utf8"))?;
            cmd.arg0(format!("-{shell_basename}"));
        };

        let noecho = self.config.get().noecho.unwrap_or(false);
        info!("about to fork subshell noecho={}", noecho);
        let mut fork = shpool_pty::fork::Fork::from_ptmx().context("forking pty")?;
        if let Ok(slave) = fork.is_child() {
            if noecho {
                if let Some(fd) = slave.borrow_fd() {
                    tty::disable_echo(fd).context("disabling echo on pty")?;
                }
            }
            for fd in consts::STDERR_FD + 1..(nix::unistd::SysconfVar::OPEN_MAX as i32) {
                let _ = nix::unistd::close(fd);
            }
            let err = cmd.exec();
            eprintln!("shell exec err: {err:?}");
            std::process::exit(1);
        }

        // spawn a background thread to reap the shell when it exits
        // and notify about the exit by closing a channel.
        let child_exit_notifier = Arc::new(ExitNotifier::new());

        // The `fork` object logically has two parts, the child pid that serves
        // as a handle to the child process, and the pty fd which allows us to
        // do IO on it. The child watcher thread only needs the child pid.
        //
        // Just cloning the fork and directly calling wait_for_exit() on it would
        // be simpler, but it would be wrong because then the destructor for the
        // cloned fork object would close the pty fd earlier than we want as the
        // child watcher thread exits. This can cause the shell->client thread
        // to read the wrong file (for example, the config file contents if the
        // config watcher reloads).
        let waitable_child_pid = fork.child_pid().ok_or(anyhow!("missing child pid"))?;
        let session_name = header.name.clone();
        let notifiable_child_exit_notifier = Arc::clone(&child_exit_notifier);
        thread::spawn(move || {
            let _s = span!(Level::INFO, "child_watcher", s = session_name, cid = conn_id).entered();

            let mut err = None;
            let mut status = 0;
            let mut unpacked_status = None;
            loop {
                // Saftey: all basic ffi, the pid is valid before this returns.
                unsafe {
                    match libc::waitpid(waitable_child_pid, &mut status, 0) {
                        0 => continue,
                        -1 => {
                            err = Some("waitpid failed");
                            break;
                        }
                        _ => {
                            if libc::WIFEXITED(status) {
                                unpacked_status = Some(libc::WEXITSTATUS(status));
                            }
                            break;
                        }
                    }
                }
            }
            if let Some(status) = unpacked_status {
                info!("child exited with status {}", status);
                notifiable_child_exit_notifier.notify_exit(status);
            } else {
                if let Some(e) = err {
                    info!("child exited without status, using 1: {:?}", e);
                } else {
                    info!("child exited without status, using 1");
                }
                notifiable_child_exit_notifier.notify_exit(1);
            }
        });

        let prompt_prefix_is_blank =
            self.config.get().prompt_prefix.as_ref().map(|p| p.is_empty()).unwrap_or(false);
        let supports_sentinels =
            header.cmd.is_none() && !prompt_prefix_is_blank && !does_not_support_sentinels(&shell);
        info!("supports_sentianls={}", supports_sentinels);

        // Inject the prompt prefix, if any. For custom commands, avoid doing this
        // since we have no idea what the command is so the shell code probably won't
        // work.
        if supports_sentinels {
            info!("injecting prompt prefix");
            let prompt_prefix = self
                .config
                .get()
                .prompt_prefix
                .clone()
                .unwrap_or(String::from(DEFAULT_PROMPT_PREFIX));
            if let Err(err) = prompt::maybe_inject_prefix(&mut fork, &prompt_prefix, &header.name) {
                warn!("issue injecting prefix: {:?}", err);
            }
        }

        let (client_connection_tx, client_connection_rx) = crossbeam_channel::bounded(0);
        let (client_connection_ack_tx, client_connection_ack_rx) = crossbeam_channel::bounded(0);
        let (tty_size_change_tx, tty_size_change_rx) = crossbeam_channel::bounded(0);
        let (tty_size_change_ack_tx, tty_size_change_ack_rx) = crossbeam_channel::bounded(0);

        let (heartbeat_tx, heartbeat_rx) = crossbeam_channel::bounded(0);
        let (heartbeat_ack_tx, heartbeat_ack_rx) = crossbeam_channel::bounded(0);

        let shell_to_client_ctl = Arc::new(Mutex::new(shell::ReaderCtl {
            client_connection: client_connection_tx,
            client_connection_ack: client_connection_ack_rx,
            tty_size_change: tty_size_change_tx,
            tty_size_change_ack: tty_size_change_ack_rx,
            heartbeat: heartbeat_tx,
            heartbeat_ack: heartbeat_ack_rx,
        }));

        let mut session_inner = shell::SessionInner {
            name: header.name.clone(),
            shell_to_client_ctl: Arc::clone(&shell_to_client_ctl),
            pty_master: fork,
            client_stream: Some(client_stream),
            config: self.config.clone(),
            shell_to_client_join_h: None,
            term_db,
            daily_messenger: Arc::clone(&self.daily_messenger),
            needs_initial_motd_dump: dump_motd_on_new_session,
            supports_sentinels,
        };
        let child_pid = session_inner.pty_master.child_pid().ok_or(anyhow!("no child pid"))?;
        session_inner.shell_to_client_join_h =
            Some(session_inner.spawn_shell_to_client(shell::ReaderArgs {
                conn_id,
                tty_size: header.local_tty_size.clone(),
                scrollback_lines: match (
                    self.config.get().output_spool_lines,
                    &self.config.get().session_restore_mode,
                ) {
                    (Some(l), _) => l,
                    (None, Some(config::SessionRestoreMode::Lines(l))) => *l as usize,
                    (None, _) => DEFAULT_OUTPUT_SPOOL_LINES,
                },
                session_restore_mode:
                    self.config.get().session_restore_mode.clone().unwrap_or_default(),
                client_connection: client_connection_rx,
                client_connection_ack: client_connection_ack_tx,
                tty_size_change: tty_size_change_rx,
                tty_size_change_ack: tty_size_change_ack_tx,
                heartbeat: heartbeat_rx,
                heartbeat_ack: heartbeat_ack_tx,
            })?);

        if let Some(ttl_secs) = header.ttl_secs {
            info!("registering session with ttl with the reaper");
            self.register_new_reapable_session
                .send((header.name.clone(), Instant::now().add(Duration::from_secs(ttl_secs))))
                .context("sending reapable session registration msg")?;
        }

        Ok(shell::Session {
            shell_to_client_ctl,
            pager_ctl: Arc::new(Mutex::new(None)),
            child_pid,
            child_exit_notifier,
            started_at: time::SystemTime::now(),
            lifecycle_timestamps: Mutex::new(shell::SessionLifecycleTimestamps::default()),
            inner: Arc::new(Mutex::new(session_inner)),
        })
    }

    /// Set up the environment for the shell, returning the right TERM value.
    #[instrument(skip_all)]
    fn build_shell_env(
        &self,
        user_info: &user::Info,
        header: &AttachHeader,
    ) -> anyhow::Result<Vec<(OsString, OsString)>> {
        let s = OsString::from;
        let config = self.config.get();
        let auth_sock = self.ssh_auth_sock_symlink(PathBuf::from(&header.name));
        let mut env = vec![
            (s("HOME"), s(&user_info.home_dir)),
            (
                s("PATH"),
                s(config
                    .initial_path
                    .as_ref()
                    .map(|x| x.as_ref())
                    .unwrap_or(DEFAULT_INITIAL_SHELL_PATH)),
            ),
            (s("SHPOOL_SESSION_NAME"), s(&header.name)),
            (
                s("SHPOOL_SESSION_DIR"),
                self.session_dir(PathBuf::from(&header.name)).into_os_string(),
            ),
            (s("SHELL"), s(&user_info.default_shell)),
            (s("USER"), s(&user_info.user)),
            (
                s("SSH_AUTH_SOCK"),
                s(auth_sock.to_str().ok_or(anyhow!("failed to convert auth sock symlink"))?),
            ),
        ];

        if let Some(xdg_runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
            env.push((s("XDG_RUNTIME_DIR"), xdg_runtime_dir));
        }

        // Most of the time, use the TERM that the user sent along in
        // the attach header. If they have an explicit TERM value set
        // in their config file, use that instead. If they have a blank
        // term in their config, don't set TERM in the spawned shell at
        // all.
        let mut term = None;
        if let Some(t) = header.local_env_get("TERM") {
            term = Some(String::from(t));
        }
        let filtered_env_pin;
        if let Some(extra_env) = config.env.as_ref() {
            term = match extra_env.get("TERM") {
                None => term,
                Some(t) if t.is_empty() => None,
                Some(t) => Some(String::from(t)),
            };

            // If the user has configured a term of "", we want
            // to make sure not to set it at all in the environment.
            // An unset TERM variable can produce a shell that generates
            // output which is easier to parse and interact with for
            // another machine. This is particularly useful for testing
            // shpool itself.
            let extra_env = if term.is_none() {
                let mut e = extra_env.clone();
                e.remove("TERM");
                filtered_env_pin = Some(e);
                filtered_env_pin.as_ref().unwrap()
            } else {
                extra_env
            };

            if !env.is_empty() {
                env.extend(extra_env.iter().map(|(k, v)| (s(k), s(v))));
            }
        }
        info!("injecting TERM into shell {:?}", term);
        if let Some(t) = &term {
            env.push((s("TERM"), s(t)));
        }

        // inject all other local variables
        for (var, val) in &header.local_env {
            if var == "TERM" || var == "SSH_AUTH_SOCK" {
                continue;
            }
            env.push((s(var), s(val)));
        }

        // parse and load /etc/environment unless we've been asked not to
        if !self.config.get().noread_etc_environment.unwrap_or(false) {
            match fs::File::open("/etc/environment") {
                Ok(f) => {
                    let pairs = etc_environment::parse_compat(io::BufReader::new(f))?;
                    for (var, val) in pairs.into_iter() {
                        env.push((var.into(), val.into()));
                    }
                }
                Err(e) => {
                    warn!("could not open /etc/environment to load env vars: {:?}", e);
                }
            }
        }
        debug!("ENV: {env:?}");

        Ok(env)
    }

    // Generate the path to the env file that is populated on every
    // attach.
    fn session_env_file<P: AsRef<Path>>(&self, session_name: P) -> PathBuf {
        self.session_dir(session_name).join("forward.env")
    }

    fn ssh_auth_sock_symlink<P: AsRef<Path>>(&self, session_name: P) -> PathBuf {
        self.session_dir(session_name).join("ssh-auth-sock.socket")
    }

    fn session_dir<P: AsRef<Path>>(&self, session_name: P) -> PathBuf {
        self.runtime_dir.join("sessions").join(session_name)
    }
}

// HACK: this is not a good way to detect shells that don't support our
// sentinel injection approach, but it is better than just hanging when a
// user tries to start one.
fn does_not_support_sentinels(shell: &str) -> bool {
    shell.ends_with("nu")
}

#[instrument(skip_all)]
fn parse_connect_header(stream: &mut UnixStream) -> anyhow::Result<ConnectHeader> {
    let header: ConnectHeader = protocol::decode_from(stream).context("parsing header")?;
    Ok(header)
}

#[instrument(skip_all)]
fn write_reply<H>(stream: &mut UnixStream, header: H) -> anyhow::Result<()>
where
    H: serde::Serialize,
{
    stream
        .set_write_timeout(Some(consts::SOCK_STREAM_TIMEOUT))
        .context("setting write timout on inbound session")?;

    let serializeable_stream = stream.try_clone().context("cloning stream handle")?;
    protocol::encode_to(&header, serializeable_stream).context("writing reply")?;

    stream.set_write_timeout(None).context("unsetting write timout on inbound session")?;

    Ok(())
}

/// check_peer makes sure that a process dialing in on the shpool
/// control socket has the same UID as the current user and that
/// both have the same executable path.
#[cfg(target_os = "linux")]
fn check_peer(sock: &UnixStream) -> anyhow::Result<()> {
    use nix::sys::socket;

    let peer_creds = socket::getsockopt(sock, socket::sockopt::PeerCredentials)
        .context("could not get peer creds from socket")?;
    let peer_uid = unistd::Uid::from_raw(peer_creds.uid());
    let self_uid = unistd::Uid::current();
    if peer_uid != self_uid {
        return Err(anyhow!("shpool prohibits connections across users"));
    }

    let peer_pid = unistd::Pid::from_raw(peer_creds.pid());
    let self_pid = unistd::Pid::this();
    let peer_exe = exe_for_pid(peer_pid).context("could not resolve exe from the pid")?;
    let self_exe = exe_for_pid(self_pid).context("could not resolve our own exe")?;
    if peer_exe != self_exe {
        warn!("attach binary differs from daemon binary");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn check_peer(sock: &UnixStream) -> anyhow::Result<()> {
    use std::os::unix::io::AsRawFd;

    let mut peer_uid: libc::uid_t = 0;
    let mut peer_gid: libc::gid_t = 0;
    // Safety: getpeereid is standard BSD FFI, all pointers are valid
    unsafe {
        if libc::getpeereid(sock.as_raw_fd(), &mut peer_uid, &mut peer_gid) != 0 {
            return Err(anyhow!(
                "could not get peer uid from socket: {}",
                io::Error::last_os_error()
            ));
        }
    }
    let peer_uid = unistd::Uid::from_raw(peer_uid);
    let self_uid = unistd::Uid::current();
    if peer_uid != self_uid {
        return Err(anyhow!("shpool prohibits connections across users"));
    }

    let mut peer_pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // Safety: getsockopt is standard POSIX FFI, all pointers and sizes are valid
    unsafe {
        if libc::getsockopt(
            sock.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            &mut peer_pid as *mut _ as *mut libc::c_void,
            &mut len,
        ) != 0
        {
            return Err(anyhow!(
                "could not get peer pid from socket: {}",
                io::Error::last_os_error()
            ));
        }
    }

    let peer_pid = unistd::Pid::from_raw(peer_pid);
    let self_pid = unistd::Pid::this();
    let peer_exe = exe_for_pid(peer_pid).context("could not resolve exe from the pid")?;
    let self_exe = exe_for_pid(self_pid).context("could not resolve our own exe")?;
    if peer_exe != self_exe {
        warn!("attach binary differs from daemon binary");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn exe_for_pid(pid: unistd::Pid) -> anyhow::Result<PathBuf> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe"))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn exe_for_pid(pid: unistd::Pid) -> anyhow::Result<PathBuf> {
    use libproc::proc_pid::pidpath;
    let path = pidpath(pid.as_raw())
        .map_err(|e| anyhow!("could not get exe path for pid {}: {:?}", pid, e))?;
    Ok(PathBuf::from(path))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellSelectionError {
    BusyShellSession,
}

impl std::fmt::Display for ShellSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{self:?}")?;
        Ok(())
    }
}

impl std::error::Error for ShellSelectionError {}
