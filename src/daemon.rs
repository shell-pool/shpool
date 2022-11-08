use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::io::{RawFd, AsRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{time, thread, process, net};
use std::os::unix::process::CommandExt;

use anyhow::{anyhow, Context};
use log::{info, error, trace, debug};
use serde_derive::Deserialize;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use super::consts;
use super::protocol;
use super::test_hooks;

// TODO(ethan): make this configurable via toml
const SSH_EXTENSION_ATTACH_WINDOW: time::Duration = time::Duration::from_secs(30);

const SUPERVISOR_POLL_DUR: time::Duration = time::Duration::from_millis(300);

#[derive(Deserialize)]
struct Config {
    // TODO(ethan): implement keepalive support
    // keepalive_secs: Option<usize>,
    
    /// norc makes it so that new shells do not load rc files when they
    /// spawn. Only works with bash.
    norc: Option<bool>,
    /// shell overrides the user's default shell
    shell: Option<String>,
    /// a table of environment variables to inject into the initial shell
    env: Option<HashMap<String, String>>,
}

pub fn run(config_file: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");
    let config_str = std::fs::read_to_string(config_file).context("reading config toml")?;
    let config: Config = toml::from_str(&config_str).context("parsing config file")?;

    let mut daemon = Daemon {
        config,
        shells: Arc::new(Mutex::new(HashMap::new())),
        ssh_extension_parker: Arc::new(SshExtensionParker {
            inner: Mutex::new(SshExtensionParkerInner {
                name: None,
                has_parked_local: false,
                has_parked_remote: false,
            }),
            cond: Condvar::new(),
        }),
    };

    let teardown_socket = socket.clone();
    ctrlc::set_handler(move || {
        info!("ctrlc handler: cleaning up socket");
        if let Err(e)= std::fs::remove_file(teardown_socket.clone()).context("cleaning up socket") {
            error!("error cleaning up socket file: {}", e);
        }

        info!("ctrlc handler: exiting");
        std::process::exit(128 + 2 /* default SIGINT exit code */);
    }).context("registering ctrlc handler")?;

    info!("listening on socket {:?}", socket);
    test_hooks::emit_event("daemon-about-to-listen");
    let listener = UnixListener::bind(&socket).context("binding to socket")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = daemon.handle_conn(stream) {
                    error!("handling new connection: {:?}", err)
                }
            }
            Err(err) => {
                error!("accepting stream: {:?}", err);
            }
        }
    }

    std::fs::remove_file(socket).context("cleaning up socket after no more incoming")?;

    Ok(())
}

struct Daemon {
    config: Config,
    /// A map from shell session names to session descriptors.
    shells: Arc<Mutex<HashMap<String, ShellSession>>>,
    /// Syncronization primitives allowing us make sure that only
    /// one thread at a time attaches using the ssh extension mechanism.
    ssh_extension_parker: Arc<SshExtensionParker>,
}

struct ShellSession {
    started_at: time::SystemTime,
    inner: Arc<Mutex<ShellSessionInner>>,
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
struct ShellSessionInner {
    pty_master: pty::fork::Fork,
    client_stream: Option<UnixStream>,
}

/// SshExtensionParker contains syncronization primitives to allow the
/// LocalCommand and RemoteCommand ssh extension threads to perform
/// a little handshake to hand off the name. ssh_config(5) leaves the
/// relative order in which these commands will execute unspecified,
/// so they might happen in either order or simultaneously. We must
/// be able to handle any possibility.
///
/// TODO(ethan): write unit tests for the various permutations of handshake
///              order.
/// TODO(ethan): Even with syncronization primitives in the daemon, I think
///              we can still get race conditions where a LocalCommand and
///              RemoteCommand from two different ssh invocations can
///              interleave. I think we are going to need some client side
///              locking in order to work around this, and even then I'm still
///              worried.
struct SshExtensionParker {
    /// The empty string indicates that there is a parked thread waiting for
    inner: Mutex<SshExtensionParkerInner>,
    cond: Condvar,
}

struct SshExtensionParkerInner {
    /// The name for the session that the thread should used to attach.
    /// Set by the LocalCommandSetName thread when it wakes up the parked
    /// RemoteCommand thread.
    name: Option<String>,
    /// True when there is a RemoteCommand thread parked.
    has_parked_remote: bool,
    /// True when there is a LocalCommand thread parked.
    has_parked_local: bool,
}

impl Daemon {
    fn handle_conn(&mut self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("handling inbound connection");
        // We want to avoid timing out while blocking the main thread.
        stream.set_read_timeout(Some(consts::SOCK_STREAM_TIMEOUT))
            .context("setting read timout on inbound session")?;

        let header = parse_connect_header(&mut stream)?;

        // Unset the read timeout before we pass things off to a
        // worker thread because it is perfectly fine for there to
        // be no new data for long periods of time when the users
        // is connected to a shell session.
        stream.set_read_timeout(None)
            .context("unsetting read timout on inbound session")?;

        match header {
            protocol::ConnectHeader::Attach(h) => self.handle_attach(stream, h),
            protocol::ConnectHeader::RemoteCommandLock => self.handle_remote_command_lock(stream),
            protocol::ConnectHeader::LocalCommandSetName(h) => self.handle_local_command_set_name(stream, h),
            protocol::ConnectHeader::List => self.handle_list(stream),
        }
    }

    fn handle_attach(&self, mut stream: UnixStream, header: protocol::AttachHeader) -> anyhow::Result<()> {
        info!("handle_attach: header={:?}", header);

        // we unwrap to propigate the poison as an unwind
        let mut shells = self.shells.lock().unwrap();

        let status: protocol::AttachStatus;
        if let Some(session) = shells.get(&header.name) {
            if let Ok(mut inner) = session.inner.try_lock() {
                info!("handle_attach taking over existing session inner={:?}", inner);
                inner.client_stream = Some(stream);

                status = protocol::AttachStatus::Attached;
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
            info!("handle_attach: creating new subshell");
            let inner = self.spawn_subshell(stream, &header)?;

            shells.insert(header.name.clone(), ShellSession {
                started_at: time::SystemTime::now(),
                inner: Arc::new(Mutex::new(inner)),
            });

            status = protocol::AttachStatus::Created;
            // fallthrough to bidi streaming
        }

        // the nested "if let" buisness is to please the borrow checker
        if let Some(session) = shells.get(&header.name) {
            let inner = Arc::clone(&session.inner);
            let shells_arc = Arc::clone(&self.shells);
            thread::spawn(move || {
                let mut child_done = false;

                if let Ok(mut inner) = inner.lock() {
                    let client_stream = match inner.client_stream.as_mut() {
                        Some(s) => s,
                        None => {
                            error!("no client stream, should be impossible");
                            return;
                        }
                    };

                    let reply_status = write_reply(client_stream, protocol::AttachReplyHeader{
                        status,
                    });
                    if let Err(e) = reply_status {
                        error!("error writing reply status: {:?}", e);
                    }

                    info!("handle_attach: starting bidi stream loop");
                    match bidi_stream(&mut inner) {
                        Ok(done) => {
                            child_done = done;
                        },
                        Err(e) => {
                            error!("error shuffling bytes: {:?}", e);
                        },
                    }
                } else {
                    error!("inernal error: failed to lock just created mutex");
                }

                if child_done {
                    info!("'{}' exited, removing from session table", header.name);
                    let mut shells = shells_arc.lock().unwrap();
                    shells.remove(&header.name);
                }
            });
        } else {
            error!("inernal error: failed to fetch just inserted session");
        }

        Ok(())
    }

    fn handle_remote_command_lock(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("handle_remote_command_lock: enter");
        let name = {
            let mut inner = self.ssh_extension_parker.inner.lock().unwrap();
            let name = if inner.has_parked_local {
                assert!(!inner.has_parked_remote, "remote: should never have two threads parked at once");

                info!("handle_remote_command_lock: there is already a parked local waiting for us");
                inner.name.take().unwrap_or_else(|| String::from(""))
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
                let (mut inner, timeout_res) = self.ssh_extension_parker.cond
                    .wait_timeout_while(inner, SSH_EXTENSION_ATTACH_WINDOW, |inner| inner.name.is_none()).unwrap();
                if timeout_res.timed_out() {
                    info!("handle_remote_command_lock: timeout");
                    write_reply(&mut stream, protocol::AttachReplyHeader{
                        status: protocol::AttachStatus::Timeout,
                    })?;
                    return Ok(())
                }

                inner.has_parked_remote = false;
                inner.name.take().unwrap_or_else(|| String::from(""))
            };

            self.ssh_extension_parker.cond.notify_one();
            name
        };

        info!("handle_remote_command_lock: becoming an attach with name '{}'", name);
        // At this point, we've gotten the name through normal means, so we
        // can just become a normal attach request.
        self.handle_attach(stream, protocol::AttachHeader { name })
    }

    fn handle_local_command_set_name(&self, mut stream: UnixStream, header: protocol::LocalCommandSetNameRequest) -> anyhow::Result<()> {
        info!("handle_local_command_set_name: header={:?}", header);
        let status = {
            let mut inner = self.ssh_extension_parker.inner.lock().unwrap();

            if inner.has_parked_remote {
                assert!(!inner.has_parked_local, "local: should never have two threads parked at once");

                info!("handle_local_command_set_name: there is a remote thread waiting to be woken");
                inner.name = Some(header.name.clone());
                self.ssh_extension_parker.cond.notify_one();

                protocol::LocalCommandSetNameStatus::Ok
            } else {
                info!("handle_local_command_set_name: no remote thread, we will have to wait ourselves");
                inner.name = Some(header.name.clone());
                inner.has_parked_local = true;
                let (mut inner, timeout_res) = self.ssh_extension_parker.cond
                    .wait_timeout_while(inner, SSH_EXTENSION_ATTACH_WINDOW, |inner| inner.name.is_none()).unwrap();
                inner.has_parked_local = false;
                if timeout_res.timed_out() {
                    info!("handle_local_command_set_name: timed out waiting for remote command");
                    protocol::LocalCommandSetNameStatus::Timeout
                } else {
                    info!("handle_local_command_set_name: finished the handshake successfully");
                    protocol::LocalCommandSetNameStatus::Ok
                }
            }
        };

        // write the reply without the lock held to avoid doin IO with a lock held
        info!("handle_local_command_set_name: status={:?} name={}", status, header.name);
        write_reply(&mut stream, protocol::LocalCommandSetNameReply{
            status: protocol::LocalCommandSetNameStatus::Ok
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

    fn spawn_subshell(
        &self,
        client_stream: UnixStream,
        header: &protocol::AttachHeader,
    ) -> anyhow::Result<ShellSessionInner> {
        let user_info = user_info()?;
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
        if let Some(env) = self.config.env.as_ref() {
            if env.len() > 0 {
                cmd.envs(env);
            }
        }

        let fork = pty::fork::Fork::from_ptmx().context("forking pty")?;
        if let Ok(slave) = fork.is_child() {
            set_term_flags(slave.as_raw_fd()).unwrap();
            let err = cmd.exec();
            eprintln!("shell exec err: {:?}", err);
            std::process::exit(1);
        }

        Ok(ShellSessionInner {
            pty_master: fork,
            client_stream: Some(client_stream),
        })
    }
}

fn set_term_flags(fd: RawFd) -> std::io::Result<()> {
    use termios::*;

    // TODO(ethan): I think to correctly support zsh I may need to disable
    // ICANON mode so we send chars in one at a time. zsh may do this automatically
    // for us though.

    let mut term = Termios::from_fd(fd)?;
    term.c_lflag &= !ECHO;

    tcsetattr(fd, TCSANOW, &term)
}

/// bidi_stream shuffles bytes between the subprocess and the client connection.
/// It returns true if the subprocess has exited, and false if it is still running.
fn bidi_stream(inner: &mut ShellSessionInner) -> anyhow::Result<bool> {
    // we take the client stream so that it gets closed when this routine
    // returns
    let mut client_stream = match inner.client_stream.take() {
        Some(s) => s,
        None => {
            return Err(anyhow!("no client stream to take for bidi streaming"))
        }
    };

    // set timeouts so we can wake up to handle cancelation correctly
    client_stream.set_nonblocking(true).context("setting client stream nonblocking")?;
    
    let mut reader_client_stream = client_stream.try_clone().context("creating reader client stream")?;
    let client_stream_m = Mutex::new(client_stream.try_clone()
                                   .context("wrapping a stream handle in mutex")?);

    let pty_master = inner.pty_master.is_parent()
        .context("internal error: executing in child fork")?;

    // A flag to indicate that outstanding threads should stop
    let stop = AtomicBool::new(false);
    // A flag to indicate if the child shell has exited
    let child_done = AtomicBool::new(false);

    thread::scope(|s| -> anyhow::Result<()> {
        // client -> shell
        let client_to_shell_h = s.spawn(|| -> anyhow::Result<()> {
            let mut master_writer = pty_master.clone();

            info!("bidi_stream: spawned client->shell thread");

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if stop.load(Ordering::Relaxed) {
                    info!("bidi_stream: client->shell: recvd stop msg (1)");
                    return Ok(())
                }

                // N.B. we don't need to muck about with chunking or anything
                // in this direction, because there is only one input stream
                // to the shell subprocess, vs the two output streams we need
                // to handle coming back the other way.
                //
                // Also, not that we don't access through the mutex because reads
                // don't need to be excluded from trampling on writes.
                let len = match reader_client_stream.read(&mut buf) {
                    Ok(l) => l,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            trace!("bidi_stream: client->shell: read: WouldBlock");
                            thread::sleep(consts::PIPE_POLL_DURATION);
                            continue;
                        }
                        return Err(e).context("reading client chunk");
                    }
                };
                if len == 0 {
                    continue;
                }

                debug!("bidi_stream: client->shell: read {} bytes", len);
                
                let mut to_write = &buf[0..len];
                debug!("bidi_stream: client->shell: created to_write='{}'", String::from_utf8_lossy(to_write));

                while to_write.len() > 0 {
                    if stop.load(Ordering::Relaxed) {
                        info!("bidi_stream: client->shell: recvd stop msg (1)");
                        return Ok(())
                    }

                    // TODO(ethan): will we even get an EWOULDBLOCK return code anymore?
                    //              the pty master file descriptor does not allow us to
                    //              mark it nonblocking.
                    let nwritten = match master_writer.write(&to_write) {
                        Ok(n) => n,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                trace!("bidi_stream: client->shell: write: WouldBlock");
                                thread::sleep(consts::PIPE_POLL_DURATION);
                                continue;
                            }
                            return Err(e).context("writing client chunk");
                        }
                    };
                    debug!("bidi_stream: client->shell: wrote {} bytes", nwritten);
                    to_write = &to_write[nwritten..];
                    trace!("bidi_stream: client->shell: to_write='{}'", String::from_utf8_lossy(to_write));
                }

                master_writer.flush().context("flushing input from client to shell")?;

                debug!("bidi_stream: client->shell: flushed chunk of len {}", len);
            }
        });

        // shell -> client
        let shell_to_client_h = s.spawn(|| -> anyhow::Result<()> {
            info!("bidi_stream: spawned shell->client thread");

            let mut master_reader = pty_master.clone();

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if stop.load(Ordering::Relaxed) {
                    info!("bidi_stream: shell->client: recvd stop msg");
                    return Ok(())
                }

                // select so we know which stream to read from, and
                // know to wake up immediately when bytes are available.
                let mut fdset = nix::sys::select::FdSet::new();
                fdset.insert(master_reader.as_raw_fd());
                let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                let nready = nix::sys::select::select(
                    None,
                    Some(&mut fdset),
                    None,
                    None,
                    Some(&mut poll_dur),
                ).context("selecting on pty master")?;
                if nready == 0 {
                    continue;
                }

                if fdset.contains(master_reader.as_raw_fd()) {
                    let len = match master_reader.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                trace!("bidi_stream: shell->client: pty master read: WouldBlock");
                                thread::sleep(consts::PIPE_POLL_DURATION);
                                continue;
                            }
                            return Err(e).context("reading pty master chunk");
                        }
                    };
                    if len == 0 {
                        trace!("bidi_stream: shell->client: 0 stdout bytes, waiting");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    }

                    let chunk = protocol::Chunk {
                        kind: protocol::ChunkKind::Data,
                        buf: &buf[..len],
                    };
                    debug!("bidi_stream: shell->client: read pty master len={} '{}'", len, String::from_utf8_lossy(chunk.buf));
                    {
                        let mut s = client_stream_m.lock().unwrap();
                        chunk.write_to(&mut *s, &stop)
                            .context("writing stdout chunk to client stream")?;
                    }
                    debug!("bidi_stream: shell->client: wrote {} pty master bytes", chunk.buf.len());
                }

                // flush immediately
                client_stream.flush().context("flushing client stream")?;
            }
        });

        // We send a steady stream of heartbeats to the client so that
        // if the connection unexpectedly goes down, we detect it immediately.
        let heartbeat_h = s.spawn(|| -> anyhow::Result<()> {
            loop {
                trace!("bidi_stream: heartbeat: checking stop_rx");
                if stop.load(Ordering::Relaxed) {
                    info!("bidi_stream: heartbeat: recvd stop msg");
                    return Ok(())
                }

                thread::sleep(consts::HEARTBEAT_DURATION);
                let chunk = protocol::Chunk {
                    kind: protocol::ChunkKind::Heartbeat,
                    buf: &[],
                };
                {
                    let mut s = client_stream_m.lock().unwrap();
                    chunk.write_to(&mut *s, &stop)
                        .context("writing heartbeat chunk to client stream")?;
                    trace!("bidi_stream: heartbeat: wrote heartbeat");
                }
            }
        });

        // poll the pty master fd to see if the child shell has exited.
        let supervisor_h = s.spawn(|| -> anyhow::Result<()> {
            loop {
                trace!("bidi_stream: supervisor: checking stop_rx");
                if stop.load(Ordering::Relaxed) {
                    info!("bidi_stream: supervisor: recvd stop msg");
                    return Ok(())
                }

                let mut master_fd = [
                    nix::poll::PollFd::new(
                        pty_master.as_raw_fd(),
                        nix::poll::PollFlags::empty()
                    ),
                ];
                let nready = nix::poll::poll(&mut master_fd, SUPERVISOR_POLL_DUR.as_millis() as i32)
                    .context("polling master fd for POLLHUP")?;
                if nready == 0 {
                    trace!("bidi_stream: supervisor: poll timeout");
                }
                if master_fd[0].revents().map(|e| e.contains(nix::poll::PollFlags::POLLHUP)).unwrap_or(false) {
                    info!("bidi_stream: supervisor: child shell exited");
                    child_done.store(true, Ordering::Release);
                    return Ok(());
                }
            }
        });

        loop {
            let c_done = child_done.load(Ordering::Acquire);
            if client_to_shell_h.is_finished() || shell_to_client_h.is_finished()
                || heartbeat_h.is_finished() || supervisor_h.is_finished() || c_done {
                debug!("bidi_stream: signaling for threads to stop: client_to_shell_finished={} shell_to_client_finished={} heartbeat_finished={} supervisor_finished={} child_done={}",
                    client_to_shell_h.is_finished(), shell_to_client_h.is_finished(),
                    heartbeat_h.is_finished(), supervisor_h.is_finished(), c_done,
                );
                stop.store(true, Ordering::Relaxed);
                break;
            }
            thread::sleep(consts::JOIN_POLL_DURATION);
        }
        debug!("bidi_stream: joining client_to_shell_h");
        match client_to_shell_h.join() {
            Ok(v) => v.context("joining client_to_shell_h")?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }
        debug!("bidi_stream: joining shell_to_client_h");
        match shell_to_client_h.join() {
            Ok(v) => v.context("joining shell_to_client_h")?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }
        debug!("bidi_stream: joining heartbeat_h");
        match heartbeat_h.join() {
            Ok(v) => v.context("joining heartbeat_h")?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }
        debug!("bidi_stream: joining supervisor_h");
        match supervisor_h.join() {
            Ok(v) => v.context("joining supervisor_h")?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }

        Ok(())
    }).context("outer thread scope")?;

    let c_done = child_done.load(Ordering::Acquire);
    if c_done {
        client_stream.shutdown(std::net::Shutdown::Both)
            .context("shutting down client stream")?;
    }

    info!("bidi_stream: done child_done={}", c_done);
    Ok(c_done)
} 


#[derive(Debug)]
struct UserInfo {
    default_shell: String,
    home_dir: String,
}

fn user_info() -> anyhow::Result<UserInfo> {
    let out = process::Command::new("/bin/sh")
        .arg("-c")
        .arg("cd ; echo \"$SHELL|$PWD\"")
        .output()
        .context("spawning subshell to determine default shell")?;
    if !out.status.success() {
        return Err(anyhow!("bad status checking for default shell: {}", out.status));
    }
    if out.stderr.len() != 0 {
        return Err(anyhow!("unexpected stderr when checking for default shell: {}",
                           String::from_utf8_lossy(&out.stderr)));
    }

    let parts = String::from_utf8(out.stdout.clone())
        .context("parsing default shell as utf8")?
        .trim().split("|").map(String::from).collect::<Vec<String>>();
    if parts.len() != 2 {
        return Err(anyhow!("could not parse output: '{}'", 
                           String::from_utf8_lossy(&out.stdout)));
    }
    Ok(UserInfo {
        default_shell: parts[0].clone(),
        home_dir: parts[1].clone(),
    })
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
