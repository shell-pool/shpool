use std::{
    collections::HashMap,
    env,
    fs,
    net,
    os,
    os::unix::{
        fs::PermissionsExt,
        io::AsRawFd,
        net::{
            UnixListener,
            UnixStream,
        },
        process::CommandExt,
    },
    path::{
        Path,
        PathBuf,
    },
    process,
    sync::{
        Arc,
        Mutex,
    },
    thread,
    time,
};

use anyhow::{
    anyhow,
    Context,
};
use crossbeam_channel::{
    RecvTimeoutError,
    TryRecvError,
};
use nix::{
    sys::signal,
    unistd::Pid,
};
use tracing::{
    error,
    info,
    instrument,
    warn,
};

use super::{
    super::{
        consts,
        protocol,
        test_hooks,
        tty,
    },
    config,
    shell,
    user,
};

const SHELL_KILL_TIMEOUT: time::Duration = time::Duration::from_millis(500);
const STDERR_FD: i32 = 2;

#[derive(Debug)]
pub struct Server {
    config: config::Config,
    /// A map from shell session names to session descriptors.
    /// We wrap this in Arc<Mutex<_>> so that we can get at the
    /// table from different threads such as the SIGWINCH thread
    /// that is spawned during the attach process, and so that
    /// handle_conn can delegate to worker threads and quickly allow
    /// the main thread to become available to accept new connections.
    shells: Arc<Mutex<HashMap<String, Box<shell::Session>>>>,
    runtime_dir: PathBuf,
}

impl Server {
    #[instrument(skip_all)]
    pub fn new(config: config::Config, runtime_dir: PathBuf) -> Arc<Self> {
        Arc::new(Server {
            config,
            shells: Arc::new(Mutex::new(HashMap::new())),
            runtime_dir,
        })
    }

    #[instrument(skip_all)]
    pub fn serve(server: Arc<Self>, listener: UnixListener) -> anyhow::Result<()> {
        test_hooks::emit("daemon-about-to-listen");
        for stream in listener.incoming() {
            info!("socket got a new connection");
            match stream {
                Ok(stream) => {
                    let server = Arc::clone(&server);
                    thread::spawn(move || {
                        if let Err(err) = server.handle_conn(stream) {
                            error!("handling new connection: {:?}", err)
                        }
                    });
                },
                Err(err) => {
                    error!("accepting stream: {:?}", err);
                },
            }
        }

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_conn(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        // We want to avoid timing out while blocking the main thread.
        stream
            .set_read_timeout(Some(consts::SOCK_STREAM_TIMEOUT))
            .context("setting read timout on inbound session")?;

        let header = parse_connect_header(&mut stream).context("parsing connect header")?;

        if let Err(err) = check_peer(&stream) {
            if let protocol::ConnectHeader::Attach(_) = header {
                write_reply(
                    &mut stream,
                    protocol::AttachReplyHeader {
                        status: protocol::AttachStatus::Forbidden(format!("{:?}", err)),
                    },
                )?;
            }
            stream
                .shutdown(net::Shutdown::Both)
                .context("closing stream")?;
            return Err(err).context("bad peer")?;
        }

        // Unset the read timeout before we pass things off to a
        // worker thread because it is perfectly fine for there to
        // be no new data for long periods of time when the users
        // is connected to a shell session.
        stream
            .set_read_timeout(None)
            .context("unsetting read timout on inbound session")?;

        match header {
            protocol::ConnectHeader::Attach(h) => self.handle_attach(stream, h),
            protocol::ConnectHeader::Detach(r) => self.handle_detach(stream, r),
            protocol::ConnectHeader::Kill(r) => self.handle_kill(stream, r),
            protocol::ConnectHeader::List => self.handle_list(stream),
            protocol::ConnectHeader::SessionMessage(header) => {
                self.handle_session_message(stream, header)
            },
        }
    }

    #[instrument(skip_all)]
    fn handle_attach(
        &self,
        mut stream: UnixStream,
        header: protocol::AttachHeader,
    ) -> anyhow::Result<()> {
        let (inner_to_stream, status) = {
            // we unwrap to propagate the poison as an unwind
            let mut shells = self.shells.lock().unwrap();

            info!("locked shells table");

            let mut status = protocol::AttachStatus::Attached;
            if let Some(session) = shells.get(&header.name) {
                info!("found entry for '{}'", header.name);
                if let Ok(mut inner) = session.inner.try_lock() {
                    info!("session '{}': locked inner", header.name);
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
                    //
                    // TODO(ethan): plumb shell hangups back to the client
                    //              somehow and have the client print out
                    //              "Session terminated because the shell exited".
                    match inner.child_exited.try_recv() {
                        Ok(_) => {
                            return Err(anyhow!("unexpected send on child_exited chan"));
                        },
                        Err(TryRecvError::Empty) => {
                            // the channel is still open so the subshell is still running
                            info!("taking over existing session inner={:?}", inner);

                            inner
                                .set_pty_size(&header.local_tty_size)
                                .context("resetting pty size on reattach")?;
                            inner.client_stream = Some(stream.try_clone()?);

                            // status is already attached
                        },
                        Err(TryRecvError::Disconnected) => {
                            // the channel is closed so we know the subshell exited
                            info!("stale inner={:?}, clobbering with new subshell", inner);
                            status = protocol::AttachStatus::Created;
                        },
                    }

                    // fallthrough to bidi streaming
                } else {
                    info!("busy shell session, doing nothing");
                    // The stream is busy, so we just inform the client and close the stream.
                    write_reply(
                        &mut stream,
                        protocol::AttachReplyHeader {
                            status: protocol::AttachStatus::Busy,
                        },
                    )?;
                    stream
                        .shutdown(net::Shutdown::Both)
                        .context("closing stream")?;
                    return Ok(());
                }
            } else {
                status = protocol::AttachStatus::Created;
            }

            if status == protocol::AttachStatus::Created {
                info!("creating new subshell");
                let session = self.spawn_subshell(stream, &header)?;

                shells.insert(header.name.clone(), Box::new(session));
                // fallthrough to bidi streaming
            }

            // return a reference to the inner session so that
            // we can work with it without the global session
            // table lock held
            if let Some(session) = shells.get(&header.name) {
                (Some(Arc::clone(&session.inner)), status)
            } else {
                (None, status)
            }
        };

        self.link_ssh_auth_sock(&header)
            .context("linking SSH_AUTH_SOCK")?;

        if let Some(inner) = inner_to_stream {
            let mut child_done = false;
            let mut inner = inner.lock().unwrap();
            let client_stream = match inner.client_stream.as_mut() {
                Some(s) => s,
                None => {
                    return Err(anyhow!("no client stream, should be impossible"));
                },
            };

            let reply_status = write_reply(client_stream, protocol::AttachReplyHeader { status });
            if let Err(e) = reply_status {
                error!("error writing reply status: {:?}", e);
            }

            let spawned_threads_tx = self.send_sigwinch_after_attach(&header)?;

            info!("starting bidi stream loop");
            match inner.bidi_stream(spawned_threads_tx) {
                Ok(done) => {
                    child_done = done;
                },
                Err(e) => {
                    error!("error shuffling bytes: {:?}", e);
                },
            }
            info!("bidi stream loop finished");

            if child_done {
                info!("'{}' exited, removing from session table", header.name);
                let mut shells = self.shells.lock().unwrap();
                shells.remove(&header.name);
            }

            info!("finished attach streaming section");
        } else {
            error!("internal error: failed to fetch just inserted session");
        }

        Ok(())
    }

    fn send_sigwinch_after_attach(
        &self,
        header: &protocol::AttachHeader,
    ) -> anyhow::Result<crossbeam_channel::Sender<()>> {
        use protocol::SessionMessageRequestPayload;

        let local_tty_size = header.local_tty_size.clone();
        let shells_arc = Arc::clone(&self.shells);
        let sess_name = header.name.clone();
        let (spawned_threads_tx, spawned_threads_rx) = crossbeam_channel::bounded(0);
        thread::spawn(move || {
            match spawned_threads_rx.recv_timeout(time::Duration::from_secs(2)) {
                Ok(()) => {
                    warn!("unexpected send on spawned_threads chan");
                    return;
                },
                Err(RecvTimeoutError::Timeout) => {
                    warn!("timed out waiting for bidi_stream threads to spawn");
                    return;
                },
                // fallthrough because channel closure indicates all threads have
                // been spawned and are ready for us
                Err(RecvTimeoutError::Disconnected) => {},
            }

            let tty_oversize = tty::Size {
                rows: local_tty_size.rows + 1,
                cols: local_tty_size.cols + 1,
            };

            // For some reason, emacs will correctly re-draw when we jiggle
            // the tty size via a ResizeRequest RPC call, but directly calling
            // local_tty_size.set_fd(...) from here does not force the redraw.
            // This doesn't make any sense because the resize RPC is just a
            // more convoluted way to make that call as far as I can tell.
            // It doesn't seem to be a timing issue since I've added some
            // long sleeps to ensure there is no race causing problems.
            {
                let shells = shells_arc.lock().unwrap();

                if let Some(session) = shells.get(&sess_name) {
                    if let Err(e) = session.rpc_call(SessionMessageRequestPayload::Resize(
                        protocol::ResizeRequest {
                            tty_size: tty_oversize,
                        },
                    )) {
                        error!("making oversize resize rpc: {:?}", e);
                    }

                    if let Err(e) = session.rpc_call(SessionMessageRequestPayload::Resize(
                        protocol::ResizeRequest {
                            tty_size: local_tty_size,
                        },
                    )) {
                        error!("making normal size resize rpc: {:?}", e);
                    }
                }
            }
        });

        Ok(spawned_threads_tx)
    }

    #[instrument(skip_all)]
    fn link_ssh_auth_sock(&self, header: &protocol::AttachHeader) -> anyhow::Result<()> {
        if self.config.nosymlink_ssh_auth_sock.unwrap_or(false) {
            return Ok(());
        }

        if let Some(ssh_auth_sock) = header.local_env_get("SSH_AUTH_SOCK") {
            let symlink = self.ssh_auth_sock_symlink(PathBuf::from(&header.name));
            fs::create_dir_all(symlink.parent().ok_or(anyhow!("no symlink parent dir"))?)
                .context("could not create directory for SSH_AUTH_SOCK symlink")?;

            let sessions_dir = symlink
                .parent()
                .and_then(|d| d.parent())
                .ok_or(anyhow!("no sessions dir"))?;
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
                "could not symlink '{:?}' to point to '{:?}'",
                symlink, ssh_auth_sock
            ))?;
        } else {
            info!("no SSH_AUTH_SOCK in client env, leaving it unlinked");
        }

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_detach(
        &self,
        mut stream: UnixStream,
        request: protocol::DetachRequest,
    ) -> anyhow::Result<()> {
        let mut not_found_sessions = vec![];
        let mut not_attached_sessions = vec![];
        {
            let shells = self.shells.lock().unwrap();
            for session in request.sessions.into_iter() {
                if let Some(s) = shells.get(&session) {
                    let reply = s.rpc_call(protocol::SessionMessageRequestPayload::Detach)?;
                    if reply == protocol::SessionMessageReply::NotAttached {
                        not_attached_sessions.push(session);
                    }
                } else {
                    not_found_sessions.push(String::from(session));
                }
            }
        }

        write_reply(
            &mut stream,
            protocol::DetachReply {
                not_found_sessions,
                not_attached_sessions,
            },
        )
        .context("writing detach reply")?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_kill(
        &self,
        mut stream: UnixStream,
        request: protocol::KillRequest,
    ) -> anyhow::Result<()> {
        let mut not_found_sessions = vec![];
        {
            let mut shells = self.shells.lock().unwrap();

            let mut to_remove = Vec::with_capacity(request.sessions.len());
            for session in request.sessions.into_iter() {
                if let Some(s) = shells.get(&session) {
                    let reply = s.rpc_call(protocol::SessionMessageRequestPayload::Detach)?;
                    if reply == protocol::SessionMessageReply::NotAttached {
                        info!("killing already detached session '{}'", session);
                    } else {
                        info!("killing attached session '{}'", session);
                    }

                    let inner = s.inner.lock().unwrap();
                    let pid = inner
                        .pty_master
                        .child_pid()
                        .ok_or(anyhow!("no child pid"))?;

                    // SIGHUP is a signal to indicate that the terminal has disconnected
                    // from a process. We can't use the normal SIGTERM graceful-shutdown
                    // signal since shells just forward those to their child process,
                    // but for shells SIGHUP serves as the graceful shutdown signal.
                    //
                    // TODO(ethan): this hangup sequence could use some work. I should
                    //              make the wait time configurable, then make the
                    //              signal sequence SIGHUP -> SIGTERM -> SIGKILL.
                    //              SIGTERM -> SIGKILL to be polite, and SIGHUP first
                    //              because SIGTERM doesn't really work on shells
                    //              and SIGHUP means "tty disconnected" which is exactly
                    //              what is going on here. There is also the systemd
                    //              shutdown usecase to consider, where systemd will
                    //              send SIGTERM -> SIGHUP -> SIGKILL for us.
                    signal::kill(Pid::from_raw(pid), Some(signal::Signal::SIGHUP))
                        .context("sending SIGKILL to child proc")?;

                    match inner.child_exited.recv_timeout(SHELL_KILL_TIMEOUT) {
                        Ok(_) => error!("internal error: unexpected send on child_exited chan"),
                        Err(RecvTimeoutError::Timeout) => {
                            signal::kill(Pid::from_raw(pid), Some(signal::Signal::SIGKILL))
                                .context("sending SIGKILL to child proc")?;
                        },
                        Err(_) => {},
                    }

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
            if to_remove.len() > 0 {
                test_hooks::emit("daemon-handle-kill-removed-shells");
            }
        }

        write_reply(&mut stream, protocol::KillReply { not_found_sessions })
            .context("writing kill reply")?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_list(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        let shells = self.shells.lock().unwrap();

        let sessions: anyhow::Result<Vec<protocol::Session>> = shells
            .iter()
            .map(|(k, v)| {
                Ok(protocol::Session {
                    name: k.to_string(),
                    started_at_unix_ms: v.started_at.duration_since(time::UNIX_EPOCH)?.as_millis()
                        as i64,
                })
            })
            .collect();
        let sessions = sessions.context("collecting running session metadata")?;

        write_reply(&mut stream, protocol::ListReply { sessions })?;

        Ok(())
    }

    #[instrument(skip_all)]
    fn handle_session_message(
        &self,
        mut stream: UnixStream,
        header: protocol::SessionMessageRequest,
    ) -> anyhow::Result<()> {
        // create a slot to store our reply so we can do
        // our IO without the lock held.
        let reply = {
            let shells = self.shells.lock().unwrap();
            if let Some(session) = shells.get(&header.session_name) {
                session.rpc_call(header.payload)?
            } else {
                protocol::SessionMessageReply::NotFound
            }
        };

        write_reply(&mut stream, reply).context("handle_session_message: writing reply")?;

        Ok(())
    }

    /// Spawn a subshell and return the sessession descriptor for it. The session is wrapped
    /// in an Arc so the inner session can hold a Weak back-reference to the session.
    #[instrument(skip_all)]
    fn spawn_subshell(
        &self,
        client_stream: UnixStream,
        header: &protocol::AttachHeader,
    ) -> anyhow::Result<shell::Session> {
        let user_info = user::info()?;
        let shell = if let Some(s) = &self.config.shell {
            s.clone()
        } else {
            user_info.default_shell.clone()
        };
        info!("user_info={:?}", user_info);

        // Build up the command we will exec while allocation is still chill.
        // We will exec this command after a fork, so we want to just inherit
        // stdout/stderr/stdin. The pty crate automatically `dup2`s the file
        // descriptors for us.
        let mut cmd = process::Command::new(&shell);
        cmd.current_dir(user_info.home_dir.clone())
            .stdin(process::Stdio::inherit())
            .stdout(process::Stdio::inherit())
            .stderr(process::Stdio::inherit())
            // The env should mostly be set up by the shell sourcing
            // rc files and whatnot, so we will start things off with
            // an environment that is blank except for a few vars we inject
            // to avoid breakage and vars the user has asked us to inject.
            .env_clear()
            .env("HOME", user_info.home_dir)
            .env("SHPOOL_SESSION_NAME", &header.name)
            .env("USER", user_info.user)
            .env("SSH_AUTH_SOCK", self.ssh_auth_sock_symlink(PathBuf::from(&header.name)));
        if self.config.norc.unwrap_or(false) && shell == "/bin/bash" {
            cmd.arg("--norc").arg("--noprofile");
        }

        if let Ok(xdg_runtime_dir) = env::var("XDG_RUNTIME_DIR") {
            cmd.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
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
        if let Some(env) = self.config.env.as_ref() {
            term = match env.get("TERM") {
                None => None,
                Some(t) if t.is_empty() => None,
                Some(t) => Some(String::from(t)),
            };

            // If the user has configured a term of "", we want
            // to make sure not to set it at all in the environment.
            // An unset TERM variable can produce a shell that generates
            // output which is easier to parse and interact with for
            // another machine. This is particularly useful for testing
            // shpool itself.
            let filtered_env_pin;
            let env = if term.is_none() {
                let mut e = env.clone();
                e.remove("TERM");
                filtered_env_pin = Some(e);
                filtered_env_pin.as_ref().unwrap()
            } else {
                env
            };

            if env.len() > 0 {
                cmd.envs(env);
            }
        }
        if let Some(t) = term {
            cmd.env("TERM", t);
        }

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
        cmd.arg0(format!("-{}", shell_basename));

        let noecho = self.config.noecho.unwrap_or(false);
        info!("about to fork subshell noecho={}", noecho);
        let fork = pty::fork::Fork::from_ptmx().context("forking pty")?;
        if let Ok(slave) = fork.is_child() {
            if noecho {
                tty::disable_echo(slave.as_raw_fd()).unwrap();
            }
            for fd in STDERR_FD + 1..(nix::unistd::SysconfVar::OPEN_MAX as i32) {
                let _ = nix::unistd::close(fd);
            }
            let err = cmd.exec();
            eprintln!("shell exec err: {:?}", err);
            std::process::exit(1);
        }

        // spawn a background thread to reap the shell when it exits
        // and notify about the exit by closing a channel.
        let (child_exited_tx, child_exited_rx) = crossbeam_channel::bounded(0);
        let waitable_child = fork.clone();
        let session_name = header.name.clone();
        thread::spawn(move || {
            // Take ownership of the sender so it gets dropped when
            // this thread exits, closing the channel.
            let _tx = child_exited_tx;

            if let Err(e) = waitable_child.wait() {
                error!("waiting to reap child shell: {:?}", e);
            }

            info!(
                "s({}): reaped child shell: {:?}",
                session_name, waitable_child
            );
        });

        let (in_tx, in_rx) = crossbeam_channel::unbounded();
        let (out_tx, out_rx) = crossbeam_channel::unbounded();
        let session_caller = Arc::new(Mutex::new(shell::SessionCaller {
            rpc_in: in_tx,
            rpc_out: out_rx,
        }));
        let session_inner = shell::SessionInner {
            name: header.name.clone(),
            caller: Arc::clone(&session_caller),
            rpc_in: in_rx,
            rpc_out: out_tx,
            child_exited: child_exited_rx,
            pty_master: fork,
            client_stream: Some(client_stream),
            config: self.config.clone(),
            // outer: sync::Weak::new(),
        };
        session_inner
            .set_pty_size(&header.local_tty_size)
            .context("setting initial pty size")?;
        Ok(shell::Session {
            caller: session_caller,
            started_at: time::SystemTime::now(),
            inner: Arc::new(Mutex::new(session_inner)),
        })
    }

    fn ssh_auth_sock_symlink(&self, session_name: PathBuf) -> PathBuf {
        self.runtime_dir
            .join("sessions")
            .join(session_name)
            .join("ssh-auth-sock.socket")
    }
}

#[instrument(skip_all)]
fn parse_connect_header(stream: &mut UnixStream) -> anyhow::Result<protocol::ConnectHeader> {
    let header: protocol::ConnectHeader =
        bincode::deserialize_from(stream).context("parsing header")?;
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
    bincode::serialize_into(serializeable_stream, &header).context("writing reply")?;

    stream
        .set_write_timeout(None)
        .context("unsetting write timout on inbound session")?;
    Ok(())
}

/// check_peer makes sure that a process dialing in on the shpool
/// control socket has the same UID as the current user and that
/// both have the same executable path.
fn check_peer(sock: &UnixStream) -> anyhow::Result<()> {
    use nix::{
        sys::socket,
        unistd,
    };

    let peer_creds = socket::getsockopt(sock.as_raw_fd(), socket::sockopt::PeerCredentials)
        .context("could not get peer creds from socket")?;
    let peer_uid = unistd::Uid::from_raw(peer_creds.uid());
    let self_uid = unistd::getuid();
    if peer_uid != self_uid {
        return Err(anyhow!("shpool prohibits connections across users"));
    }

    let peer_pid = unistd::Pid::from_raw(peer_creds.pid());
    let self_pid = unistd::getpid();
    let peer_exe = exe_for_pid(peer_pid).context("could not resolve exe from the pid")?;
    let self_exe = exe_for_pid(self_pid).context("could not resolve our own exe")?;
    if peer_exe != self_exe {
        return Err(anyhow!(
            "shpool prohibits connecting to a daemon with a different exe"
        ));
    }

    Ok(())
}

fn exe_for_pid(pid: Pid) -> anyhow::Result<PathBuf> {
    let path = std::fs::read_link(format!("/proc/{}/exe", pid))?;
    Ok(path)
}
