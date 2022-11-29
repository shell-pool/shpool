use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar};
use std::os::unix::process::CommandExt;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::path::Path;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::{env, time, thread, process, net};

use crossbeam_channel::TryRecvError;
use anyhow::{anyhow, Context};
use log::{error, info, warn};

use super::{config, ssh_plugin, shell, user};
use super::super::{consts, protocol, tty, test_hooks};

// controls how long we wait on attempted sends and receives
// when sending a message to a running session
const DEFAULT_SSH_HANDSHAKE_TIMEOUT_MS: u64 = 30 * 1000;

pub struct Server {
    config: config::Config,
    /// A map from shell session names to session descriptors.
    shells: Arc<Mutex<HashMap<String, shell::Session>>>,
    /// Syncronization primitives allowing us make sure that only
    /// one thread at a time attaches using the ssh extension mechanism.
    ssh_extension_parker: Arc<ssh_plugin::Parker>,
}

impl Server {
    pub fn new(config: config::Config) -> Arc<Self> {
        Arc::new(Server {
            config,
            shells: Arc::new(Mutex::new(HashMap::new())),
            ssh_extension_parker: Arc::new(ssh_plugin::Parker {
                inner: Mutex::new(ssh_plugin::ParkerInner {
                    metadata: None,
                    has_parked_local: false,
                    has_parked_remote: false,
                }),
                cond: Condvar::new(),
            }),
        })
    }

    pub fn serve(server: Arc<Self>, listener: UnixListener) -> anyhow::Result<()>
    {
        info!("listening on socket");
        test_hooks::emit_event("daemon-about-to-listen");
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
                }
                Err(err) => {
                    error!("accepting stream: {:?}", err);
                }
            }
        }

        Ok(())
    }

    fn handle_conn(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("handling inbound connection");
        // We want to avoid timing out while blocking the main thread.
        stream.set_read_timeout(Some(consts::SOCK_STREAM_TIMEOUT))
            .context("setting read timout on inbound session")?;

        let header = parse_connect_header(&mut stream)
            .context("parsing connect header")?;

        // Unset the read timeout before we pass things off to a
        // worker thread because it is perfectly fine for there to
        // be no new data for long periods of time when the users
        // is connected to a shell session.
        stream.set_read_timeout(None)
            .context("unsetting read timout on inbound session")?;

        match header {
            protocol::ConnectHeader::Attach(h) =>
                self.handle_attach(stream, h),
            protocol::ConnectHeader::Detach(r) =>
                self.handle_detach(stream, r),
            protocol::ConnectHeader::RemoteCommandLock =>
                self.handle_remote_command_lock(stream),
            protocol::ConnectHeader::LocalCommandSetMetadata(r) =>
                self.handle_local_command_set_metadata(stream, r),
            protocol::ConnectHeader::List =>
                self.handle_list(stream),
            protocol::ConnectHeader::SessionMessage(header) =>
                self.handle_session_message(stream, header),
        }
    }

    fn handle_attach(&self, mut stream: UnixStream, header: protocol::AttachHeader) -> anyhow::Result<()> {
        info!("handle_attach: header={:?}", header);

        let (inner_to_stream, status) = {
            // we unwrap to propigate the poison as an unwind
            let mut shells = self.shells.lock().unwrap();

            info!("handle_attach: locked shells table");

            let mut status = protocol::AttachStatus::Attached;
            if let Some(session) = shells.get(&header.name) {
                info!("handle_attach: found entry for '{}'", header.name);
                if let Ok(mut inner) = session.inner.try_lock() {
                    info!("handle_attach: session '{}': locked inner", header.name);
                    // We have an existing session in our table, but the subshell
                    // proc might have exited in the mean time, for example if the
                    // user typed `exit` right before the connection dropped there
                    // could be a zombie entry in our session table. We need to
                    // re-check whether the subshell has exited before taking this over.
                    match inner.child_exited.try_recv() {
                        Ok(_) => {
                            return Err(anyhow!("unexpected send on child_exited chan"));
                        }
                        Err(TryRecvError::Empty) => {
                            // the channel is still open so the subshell is still running
                            info!("handle_attach: taking over existing session inner={:?}", inner);
                            inner.set_pty_size(&header.local_tty_size)
                                .context("resetting pty size on reattach")?;
                            inner.client_stream = Some(stream.try_clone()?);

                            // status is already attached
                        }
                        Err(TryRecvError::Disconnected) => {
                            // the channel is closed so we know the subshell exited
                            info!("handle_attach: stale inner={:?}, clobbering with new subshell", inner);
                            status = protocol::AttachStatus::Created;
                        }
                    }

                    // fallthrough to bidi streaming
                } else {
                    info!("handle_attach: busy shell session, doing nothing");
                    // The stream is busy, so we just inform the client and close the stream.
                    write_reply(&mut stream, protocol::AttachReplyHeader{
                        status: protocol::AttachStatus::Busy,
                    })?;
                    stream.shutdown(net::Shutdown::Both).context("closing stream")?;
                    return Ok(())
                }
            } else {
                status = protocol::AttachStatus::Created;
            }

            if status == protocol::AttachStatus::Created {
                info!("handle_attach: creating new subshell");
                let (rpc_in, rpc_out, inner) = self.spawn_subshell(stream, &header)?;

                shells.insert(header.name.clone(), shell::Session {
                    rpc_in,
                    rpc_out,
                    started_at: time::SystemTime::now(),
                    inner: Arc::new(Mutex::new(inner)),
                });
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

        if let Some(inner) = inner_to_stream {
            let mut child_done = false;
            let mut inner = inner.lock().unwrap();
            let client_stream = match inner.client_stream.as_mut() {
                Some(s) => s,
                None => {
                    return Err(anyhow!("no client stream, should be impossible"));
                }
            };

            let reply_status = write_reply(client_stream, protocol::AttachReplyHeader{
                status,
            });
            if let Err(e) = reply_status {
                error!("error writing reply status: {:?}", e);
            }

            info!("handle_attach: starting bidi stream loop");
            match inner.bidi_stream() {
                Ok(done) => {
                    child_done = done;
                },
                Err(e) => {
                    error!("error shuffling bytes: {:?}", e);
                },
            }

            if child_done {
                info!("'{}' exited, removing from session table", header.name);
                let mut shells = self.shells.lock().unwrap();
                shells.remove(&header.name);
            }
        } else {
            error!("internal error: failed to fetch just inserted session");
        }

        Ok(())
    }

    fn handle_detach(&self, mut stream: UnixStream, request: protocol::DetachRequest) -> anyhow::Result<()> {

        let mut not_found_sessions = vec![];
        let mut not_attached_sessions = vec![];
        {
            let shells = self.shells.lock().unwrap();
            for session in request.sessions.iter() {
                if let Some(s) = shells.get(session) {
                    let reply = s.rpc_call(protocol::SessionMessageRequestPayload::Detach)?;
                    if reply == protocol::SessionMessageReply::NotAttached {
                        not_attached_sessions.push(String::from(session));
                    }
                } else {
                    not_found_sessions.push(String::from(session));
                }
            }
        }

        write_reply(&mut stream, protocol::DetachReply {
            not_found_sessions,
            not_attached_sessions,
        }).context("writing detach reply")?;

        Ok(())
    }

    fn handle_remote_command_lock(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("handle_remote_command_lock: enter");
        let metadata = {
            let mut inner = self.ssh_extension_parker.inner.lock().unwrap();

            // if the metadata has expired, clobber it
            let attach_timeout = time::Duration::from_millis(
                self.config.ssh_handshake_timeout_ms
                    .unwrap_or(DEFAULT_SSH_HANDSHAKE_TIMEOUT_MS));
            let mut clobber_metadata = false;
            if let Some(md) = &inner.metadata {
                info!("handle_remote_command_lock: checking to see if existing metadata is valid");
                if time::Instant::now().duration_since(md.set_at) > attach_timeout {
                    clobber_metadata = true;
                }
            }
            if clobber_metadata {
                info!("handle_remote_command_lock: clobbering metadata");
                inner.metadata = None;
            }

            let metadata = if inner.has_parked_local {
                info!("handle_remote_command_lock: there is already a parked local waiting for us");
                inner.metadata.take().unwrap_or_else(|| ssh_plugin::Metadata::default())
            } else {
                if inner.has_parked_remote {
                    info!("handle_remote_command_lock: remote parking slot full");
                    write_reply(&mut stream, protocol::AttachReplyHeader{
                        status: protocol::AttachStatus::SshExtensionParkingSlotFull,
                    })?;
                    return Ok(());
                }

                info!("handle_remote_command_lock: about to park");
                inner.has_parked_remote = true;

                let attach_timeout = time::Duration::from_millis(
                    self.config.ssh_handshake_timeout_ms
                        .unwrap_or(DEFAULT_SSH_HANDSHAKE_TIMEOUT_MS));

                let (mut inner, timeout_res) =
                    self.ssh_extension_parker.cond.wait_timeout_while(
                        inner, attach_timeout, |inner| inner.metadata.is_none()).unwrap();
                if timeout_res.timed_out() {
                    info!("handle_remote_command_lock: timeout");
                    write_reply(&mut stream, protocol::AttachReplyHeader{
                        status: protocol::AttachStatus::Timeout,
                    })?;
                    return Ok(())
                }

                inner.has_parked_remote = false;
                inner.metadata.take().unwrap_or_else(|| ssh_plugin::Metadata::default())
            };

            self.ssh_extension_parker.cond.notify_one();
            metadata
        };

        let tty_size = match tty::Size::from_fd(0) {
            Ok(s) => s,
            Err(e) => {
                warn!("stdin is not a tty, using default size (err: {:?})", e);
                tty::Size { rows: 24, cols: 80 }
            }
        };

        info!("handle_remote_command_lock: becoming an attach with {:?}", metadata);
        // At this point, we've gotten the name through normal means, so we
        // can just become a normal attach request.
        self.handle_attach(stream, protocol::AttachHeader {
            name: metadata.name,
            term: metadata.term,
            local_tty_size: tty_size,
        })
    }

    fn handle_local_command_set_metadata(
        &self,
        mut stream: UnixStream,
        header: protocol::SetMetadataRequest,
    ) -> anyhow::Result<()> {
        info!("handle_local_command_set_metadata: header={:?}", header);
        let status = {
            let mut inner = self.ssh_extension_parker.inner.lock().unwrap();

            if inner.has_parked_remote {
                assert!(!inner.has_parked_local, "local: should never have two threads parked at once");

                info!("handle_local_command_set_metadata: there is a remote thread waiting to be woken");
                inner.metadata = Some(ssh_plugin::Metadata {
                    name: header.name.clone(),
                    term: header.term.clone(),
                    set_at: time::Instant::now(),
                });
                self.ssh_extension_parker.cond.notify_one();

                protocol::LocalCommandSetMetadataStatus::Ok
            } else {
                info!("handle_local_command_set_metadata: no remote thread, we will have to wait ourselves");
                inner.metadata = Some(ssh_plugin::Metadata {
                    name: header.name.clone(),
                    term: header.term.clone(),
                    set_at: time::Instant::now(),
                });
                inner.has_parked_local = true;

                let attach_timeout = time::Duration::from_millis(
                    self.config.ssh_handshake_timeout_ms
                        .unwrap_or(DEFAULT_SSH_HANDSHAKE_TIMEOUT_MS));

                let (mut inner, timeout_res) = self.ssh_extension_parker.cond
                    .wait_timeout_while(inner, attach_timeout, |inner| inner.metadata.is_some()).unwrap();
                inner.has_parked_local = false;
                if timeout_res.timed_out() {
                    info!("handle_local_command_set_metadata: timed out waiting for remote command");
                    protocol::LocalCommandSetMetadataStatus::Timeout
                } else {
                    info!("handle_local_command_set_metadata: finished the handshake successfully");
                    protocol::LocalCommandSetMetadataStatus::Ok
                }
            }
        };

        // write the reply without the lock held to avoid doin IO with a lock held
        info!("handle_local_command_set_metadata: status={:?} name={}", status, header.name);
        write_reply(&mut stream, protocol::LocalCommandSetMetadataReply{
            status,
        })?;
        return Ok(())
    }

    fn handle_list(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("handle_list: enter");

        let shells = self.shells.lock().unwrap();

        let sessions: anyhow::Result<Vec<protocol::Session>> = shells
            .iter()
            .map(|(k, v)| {
                Ok(protocol::Session {
                    name: k.to_string(),
                    started_at_unix_ms: v.started_at
                                            .duration_since(time::UNIX_EPOCH)?
                                            .as_millis() as i64,
                })
            }).collect();
        let sessions = sessions.context("collecting running session metadata")?;

        write_reply(&mut stream, protocol::ListReply { sessions })?;

        Ok(())
    }

    fn handle_session_message(&self, mut stream: UnixStream, header: protocol::SessionMessageRequest) -> anyhow::Result<()> {
        info!("handle_session_message: header={:?}", header);

        // create a slot to store our reply so we can do
        // our IO without the lock held.
        let reply: protocol::SessionMessageReply;

        {
            let shells = self.shells.lock().unwrap();
            if let Some(session) = shells.get(&header.session_name) {
                reply = session.rpc_call(header.payload)?;
            } else {
                reply = protocol::SessionMessageReply::NotFound;
            }
        }

        write_reply(&mut stream, reply)
            .context("handle_session_message: writing reply")?;

        Ok(())
    }

    fn spawn_subshell(
        &self,
        client_stream: UnixStream,
        header: &protocol::AttachHeader,
    ) -> anyhow::Result<(
        crossbeam_channel::Sender<protocol::SessionMessageRequestPayload>,
        crossbeam_channel::Receiver<protocol::SessionMessageReply>,
        shell::SessionInner
    )> {
        let user_info = user::info()?;
        let shell = if let Some(s) = &self.config.shell {
            s.clone()
        } else {
            user_info.default_shell.clone()
        };
        info!("spawn_subshell: user_info={:?}", user_info);

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
            // an environment that is blank except for a little marker
            // environment variable that people can hook into for scripts
            // and whatnot.
            .env_clear()
            .env("HOME", user_info.home_dir)
            .env("SHPOOL_SESSION_NAME", &header.name);
        if self.config.norc.unwrap_or(false) && shell == "/bin/bash" {
            cmd.arg("--norc").arg("--noprofile");
        }

        if let Ok(xdg_runtime_dir) = env::var("XDG_RUNTIME_DIR") {
            cmd.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
        }

        let mut term = header.term.to_string();
        if let Some(env) = self.config.env.as_ref() {
            if let Some(t) = env.get("TERM") {
                term = t.to_string();
            }

            let filtered_env_pin;
            let env = if term == "" {
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
        if term != "" {
            cmd.env("TERM", term);
        }

        // spawn the shell as a login shell by setting
        // arg0 to be the basename of the shell path
        // proceeded with a "-". You can see sshd doing the
        // same thing if you look in the session.c file of
        // openssh.
        let shell_basename =
            Path::new(&shell).file_name()
                .ok_or(anyhow!("error building login shell indicator"))?
                .to_str().ok_or(anyhow!("error parsing shell name as utf8"))?;
        cmd.arg0(format!("-{}", shell_basename));

        let fork = pty::fork::Fork::from_ptmx().context("forking pty")?;
        if let Ok(slave) = fork.is_child() {
            if self.config.noecho.unwrap_or(false) {
                tty::disable_echo(slave.as_raw_fd()).unwrap();
            }
            let err = cmd.exec();
            eprintln!("shell exec err: {:?}", err);
            std::process::exit(1);
        }

        // spawn a background thread to reap the shell when it exits
        // and notify about the exit by closing a channel.
        let (child_exited_tx, child_exited_rx) = crossbeam_channel::bounded(0);
        let waitable_child = fork.clone();
        thread::spawn(move || {
            // Take ownership of the sender so it gets dropped when
            // this thread exits, closing the channel.
            let _tx = child_exited_tx;

            match waitable_child.wait() {
                Ok(_) => {} // fallthrough
                Err(e) => {
                    error!("waiting to reap child shell: {:?}", e);
                }
            }

            info!("reaped child shell: {:?}", waitable_child);
        });

        let (in_tx, in_rx) = crossbeam_channel::unbounded();
        let (out_tx, out_rx) = crossbeam_channel::unbounded();
        let session = shell::SessionInner {
            rpc_in: in_rx,
            rpc_out: out_tx,
            child_exited: child_exited_rx,
            pty_master: fork,
            client_stream: Some(client_stream),
        };
        session.set_pty_size(&header.local_tty_size).context("setting initial pty size")?;
        Ok((in_tx, out_rx, session))
    }
}


fn parse_connect_header(stream: &mut UnixStream) -> anyhow::Result<protocol::ConnectHeader> {
    let length_prefix = stream.read_u32::<LittleEndian>()
        .context("reading header length prefix")?;
    let mut buf: Vec<u8> = vec![0; length_prefix as usize];
    stream.read_exact(&mut buf).context("reading header buf")?;

    let header: protocol::ConnectHeader =
        rmp_serde::from_slice(&buf).context("parsing header")?;
    Ok(header)
}

fn write_reply<H>(
    stream: &mut UnixStream,
    header: H,
) -> anyhow::Result<()>
    where H: serde::Serialize
{
    stream.set_write_timeout(Some(consts::SOCK_STREAM_TIMEOUT))
        .context("setting write timout on inbound session")?;

    let buf = rmp_serde::to_vec(&header).context("formatting reply header")?;
    stream.write_u32::<LittleEndian>(buf.len() as u32)
        .context("writing reply length prefix")?;
    stream.write_all(&buf).context("writing reply header")?;

    stream.set_write_timeout(None)
        .context("unsetting write timout on inbound session")?;
    Ok(())
}
