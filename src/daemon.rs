use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{time, thread, process, net};

use anyhow::{anyhow, Context};
use log::{info, error, trace, debug};
use serde_derive::Deserialize;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crossbeam::channel;

use super::consts;
use super::protocol;

#[derive(Deserialize)]
struct Config {
    // TODO(ethan): implement keepalive support
    // keepalive_secs: Option<usize>,
}


struct ShellSession {
    started_at: time::SystemTime,
    inner: Arc<Mutex<ShellSessionInner>>,
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
#[derive(Debug)]
struct ShellSessionInner {
    shell_proc: process::Child,
    client_stream: UnixStream,
}

pub fn run(config_file: String, socket: PathBuf) -> anyhow::Result<()> {
    let config_str = std::fs::read_to_string(config_file).context("reading config toml")?;
    let _config: Config = toml::from_str(&config_str).context("parsing config file")?;


    let mut daemon = Daemon {
        shells: Arc::new(Mutex::new(HashMap::new())),
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
    // a map from shell session names to session descriptors
    shells: Arc<Mutex<HashMap<String, ShellSession>>>,
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
                inner.client_stream = stream;

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
            let inner = spawn_subshell(stream, &header)?;

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
                    let reply_status = write_reply(&mut inner.client_stream, protocol::AttachReplyHeader{
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

    fn handle_list(&self, mut stream: UnixStream) -> anyhow::Result<()> {
        info!("responding to list request");

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
}

/// bidi_stream shuffles bytes between the subprocess and the client connection.
/// It returns true if the subprocess has exited, and false if it is still running.
fn bidi_stream(inner: &mut ShellSessionInner) -> anyhow::Result<bool> {
    // set timeouts so we can wake up to handle cancelation correctly
    inner.client_stream.set_nonblocking(true).context("setting client stream nonblocking")?;
    
    // clone the client stream handle so it won't be borrowed in two
    // closures
    /* TODO(ethan): clean up
    let mut client_read_stream = inner.client_stream.try_clone()
        .context("cloning client read stream")?;
    let mut heartbeat_write_stream = inner.client_stream.try_clone()
        .context("cloning heartbeat write stream")?;
        */
    let client_stream = Mutex::new(inner.client_stream.try_clone()
                                   .context("wrapping a stream handle in mutex")?);

    // create a channel so we can make sure both worker threads exit
    // if one of them does
    let (stop_tx, stop_rx) = channel::bounded(3);

    let mut stdin = inner.shell_proc.stdin.take().ok_or(anyhow!("missing stdin"))?;
    let mut stdout = inner.shell_proc.stdout.take().ok_or(anyhow!("missing stdout"))?;
    let mut stderr = inner.shell_proc.stderr.take().ok_or(anyhow!("missing stderr"))?;

    for fd in vec![stdin.as_raw_fd(), stdout.as_raw_fd(), stderr.as_raw_fd()].iter() {
        nix::fcntl::fcntl(
            *fd,
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        ).context("setting stdin nonblocking")?;
    }

    let mut child_done = false;
    let thread_result = thread::scope(|s| -> anyhow::Result<()> {
        // client -> shell
        let client_to_shell_h = s.spawn(|| -> anyhow::Result<()> {
            info!("bidi_stream: spawned client->shell thread");

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                // N.B. we don't need to muck about with chunking or anything
                // in this direction, because there is only one input stream
                // to the shell subprocess, vs the two output streams we need
                // to handle coming back the other way.
                let len;
                {
                    let mut s = client_stream.lock().unwrap();
                    len = match s.read(&mut buf) {
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
                }

                debug!("bidi_stream: client->shell: read {} bytes", len);
                
                let mut to_write = &buf[0..len];
                trace!("bidi_stream: client->shell: created to_write={:?}", to_write);

                while to_write.len() > 0 {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

                    let nwritten = match stdin.write(&to_write) {
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
                    trace!("bidi_stream: client->shell: to_write={:?}", to_write);
                }

                stdin.flush().context("flushing stdin")?;
            }
        });

        // shell -> client
        let shell_to_client_h = s.spawn(|| -> anyhow::Result<()> {
            info!("bidi_stream: spawned shell->client thread");

            let mut buf: Vec<u8> = vec![0; consts::BUF_SIZE];

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                // select so we know which stream to read from, and
                // know to wake up immediately when bytes are available.
                let mut shell_out_set = nix::sys::select::FdSet::new();
                shell_out_set.insert(stdout.as_raw_fd());
                shell_out_set.insert(stderr.as_raw_fd());
                let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                let nready = nix::sys::select::select(
                    None,
                    Some(&mut shell_out_set),
                    None,
                    None,
                    Some(&mut poll_dur),
                ).context("selecting on stdout")?;
                if nready == 0 {
                    continue;
                }

                if shell_out_set.contains(stdout.as_raw_fd()) {
                    let len = match stdout.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                trace!("bidi_stream: shell->client: stdout read: WouldBlock");
                                thread::sleep(consts::PIPE_POLL_DURATION);
                                continue;
                            }
                            return Err(e).context("reading stdout chunk");
                        }
                    };
                    if len == 0 {
                        trace!("bidi_stream: shell->client: 0 stdout bytes, waiting");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    }

                    debug!("bidi_stream: shell->client: read {} stdout bytes", len);

                    let chunk = protocol::Chunk {
                        kind: protocol::ChunkKind::Stdout,
                        buf: &buf[..len],
                    };
                    {
                        let mut s = client_stream.lock().unwrap();
                        chunk.write_to(&mut *s, stop_rx.clone())
                            .context("writing stdout chunk to client stream")?;
                    }
                    debug!("bidi_stream: shell->client: wrote {} stdout bytes", chunk.buf.len());
                }

                if shell_out_set.contains(stderr.as_raw_fd()) {
                    let len = match stderr.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                trace!("bidi_stream: shell->client: stderr read: WouldBlock");
                                thread::sleep(consts::PIPE_POLL_DURATION);
                                continue;
                            }
                            return Err(e).context("reading stderr chunk");
                        }
                    };
                    if len == 0 {
                        trace!("bidi_stream: shell->client: 0 stderr bytes, waiting");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    }

                    debug!("bidi_stream: shell->client: read {} stderr bytes", len);

                    let chunk = protocol::Chunk {
                        kind: protocol::ChunkKind::Stderr,
                        buf: &buf[..len],
                    };
                    chunk.write_to(&mut inner.client_stream, stop_rx.clone())
                        .context("writing stderr chunk to client stream")?;
                    debug!("bidi_stream: shell->client: wrote {} stderr bytes", chunk.buf.len());
                }

                // flush immediately
                inner.client_stream.flush().context("flushing client stream")?;
            }
        });

        // We send a steady stream of heartbeats to the client so that
        // if the connection unexpectedly goes down, we detect it immediately.
        let heartbeat_h = s.spawn(|| -> anyhow::Result<()> {
            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                thread::sleep(consts::HEARTBEAT_DURATION);
                let chunk = protocol::Chunk {
                    kind: protocol::ChunkKind::NoOp,
                    buf: &[],
                };
                {
                    let mut s = client_stream.lock().unwrap();
                    chunk.write_to(&mut *s, stop_rx.clone())
                        .context("writing heartbeat chunk to client stream")?;
                }
                trace!("bidi_stream: heartbeat: wrote heartbeat");
            }
        });

        loop {
            if let Some(status) = inner.shell_proc.try_wait().context("checking on child proc")? {
                child_done = true;
                info!("bidi_stream: shell process exited with status {}", status);
            }
            if client_to_shell_h.is_finished() || shell_to_client_h.is_finished()
                || heartbeat_h.is_finished() || child_done {
                debug!("bidi_stream: signaling for threads to stop: client_to_shell_finished={} shell_to_client_finished={} heartbeat_finished={} child_done={}",
                    client_to_shell_h.is_finished(), shell_to_client_h.is_finished(),
                    heartbeat_h.is_finished(), child_done,
               );
                stop_tx.send(()).context("sending stop msg 1")?;
                stop_tx.send(()).context("sending stop msg 2")?;
                stop_tx.send(()).context("sending stop msg 3")?;
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

        Ok(())
    }).context("outer thread scope");

    // Replace the I/O handles for the subprocess so that they can be picked up
    // when the user dials in to attach again.
    inner.shell_proc.stdin = Some(stdin);
    inner.shell_proc.stdout = Some(stdout);
    inner.shell_proc.stderr = Some(stderr);

    // Delay error propigation until after we have replaced the I/O
    // handles.
    thread_result?;

    info!("bidi_stream: done child_done={}", child_done);
    Ok(child_done)
} 

fn spawn_subshell(
    client_stream: UnixStream,
    header: &protocol::AttachHeader,
) -> anyhow::Result<ShellSessionInner> {
    let user_info = user_info()?;
    info!("user_info={:?}", user_info);
    let child = process::Command::new(user_info.default_shell)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .current_dir(user_info.home_dir.clone())
        // The env should mostly be set up by the shell sourcing
        // rc files and whatnot, so we will start things off with
        // an environment that is blank except for a little marker
        // environment variable that people can hook into for scripts
        // and whatnot.
        .env_clear()
        .env("HOME", user_info.home_dir)
        .env("SHPOOL_SESSION_NAME", &header.name)
        .arg("-i") // TODO(ethan): HACK: find some way to indicate this in a
                   //              shell agnostic way
        .arg("-l") // TODO(ethan): HACK: we should be using a pty rather than forcing
                   //              a login shell with a shell-specific flag.
        .spawn()
        .context("spawning subshell")?;

    Ok(ShellSessionInner {
        shell_proc: child,
        client_stream,
    })
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
