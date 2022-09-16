use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::{env, time, thread, process, net};

use anyhow::{anyhow, Context};
use log::{info, error};
use serde_derive::Deserialize;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crossbeam::channel;

use super::protocol;

const SOCK_STREAM_TIMEOUT: time::Duration = time::Duration::from_millis(200);
const JOIN_POLL_DURATION: time::Duration = time::Duration::from_millis(100);

const PIPE_POLL_MILLIS: u64 = 100;
const PIPE_POLL_DURATION: time::Duration = time::Duration::from_millis(PIPE_POLL_MILLIS);
const PIPE_POLL_DURATION_TIMEVAL: nix::sys::time::TimeVal =
    nix::sys::time::TimeVal::new(0, 1000 * (PIPE_POLL_MILLIS as nix::sys::time::suseconds_t));

const BUF_SIZE: usize = 1024 * 16; // 16k buffers

#[derive(Deserialize)]
struct Config {
    keepalive_secs: Option<usize>,
}


struct ShellSession {
    started_at: time::SystemTime,
    inner: Box<Mutex<ShellSessionInner>>,
}

/// ShellSessionInner contains values that the pipe thread needs to be
/// able to mutate and fully control.
struct ShellSessionInner {
    shell_proc: process::Child,
    client_stream: UnixStream,
}

pub fn run(config_file: String, socket: Option<String>) -> anyhow::Result<()> {
    let config_str = std::fs::read_to_string(config_file).context("reading config toml")?;
    let _config: Config = toml::from_str(&config_str).context("parsing config file")?;

    let socket = match socket {
        Some(s) => PathBuf::from(s),
        None => {
            let runtime_dir = env::var("XDG_RUNTIME_DIR").context("getting runtime dir")?;
            PathBuf::from(runtime_dir).join("shpool.socket")
        },
    };
    info!("listening on socket {:?}", socket);

    let mut daemon = Daemon {
        shells: HashMap::new(),
    };

    let listener = UnixListener::bind(socket).context("binding to socket")?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::scope(|s| {
                    let d = &mut daemon;
                    s.spawn(move || {
                        if let Err(err) = d.handle_attach(stream) {
                            error!("handling attach: {:?}", err)
                        }
                    });
                })
            }
            Err(err) => {
                error!("accepting stream: {:?}", err);
            }
        }
    }

    Ok(())
}

struct Daemon {
    // a map from shell session names to session descriptors
    shells: HashMap<String, ShellSession>,
}

impl Daemon {
    fn handle_attach(&mut self, mut stream: UnixStream) -> anyhow::Result<()> {
        // We want to avoid timing out while blocking the main thread.
        stream.set_read_timeout(Some(SOCK_STREAM_TIMEOUT))
            .context("setting read timout on inbound session")?;

        let header = parse_attach_header(&mut stream)?;

        // Unset the read timeout before we pass things off to a
        // worker thread because it is perfectly fine for there to
        // be no new data for long periods of time when the users
        // is connected to a shell session.
        stream.set_read_timeout(None)
            .context("unsetting read timout on inbound session")?;

        if let Some(session) = self.shells.get(&header.name) {
            if let Ok(mut inner) = session.inner.try_lock() {
                inner.client_stream = stream;
                
                write_attach_reply_header(&mut inner.client_stream, protocol::AttachReplyHeader{
                    status: protocol::AttachStatus::Attached,
                })?;

                bidi_stream(&mut inner)?;
            } else {
                // The stream is busy, so we just inform the client and close the stream.
                write_attach_reply_header(&mut stream, protocol::AttachReplyHeader{
                    status: protocol::AttachStatus::Busy,
                })?;
                stream.shutdown(net::Shutdown::Both).context("closing stream")?;
                return Ok(())
            }
        } else {
            let inner = spawn_subshell(stream)?;

            self.shells.insert(header.name.clone(), ShellSession {
                started_at: time::SystemTime::now(),
                inner: Box::new(Mutex::new(inner)),
            });

            // the nested "if let" buisness is to please the borrow checker
            if let Some(session) = self.shells.get(&header.name) {
                if let Ok(mut inner) = session.inner.lock() {
                    write_attach_reply_header(&mut inner.client_stream, protocol::AttachReplyHeader{
                        status: protocol::AttachStatus::Created,
                    })?;

                    bidi_stream(&mut inner)?;
                } else {
                    error!("inernal error: failed to lock just created mutex");
                }
            } else {
                error!("inernal error: failed to fetch just inserted session");
            }

        }

        Ok(())
    }
}

fn bidi_stream(inner: &mut ShellSessionInner) -> anyhow::Result<()> {
    // set timeouts so we can wake up to handle cancelation correctly
    inner.client_stream.set_read_timeout(Some(PIPE_POLL_DURATION))?;
    inner.client_stream.set_write_timeout(Some(PIPE_POLL_DURATION))?;
    
    // clone the client stream handle so it won't be borrowed in two
    // closures
    let mut client_read_stream = inner.client_stream.try_clone().context("cloning client read stream")?;

    // create a channel so we can make sure both worker threads exit
    // if one of them does
    let (stop_tx, stop_rx) = channel::bounded(1);

    thread::scope(|s| -> anyhow::Result<()> {
        // client -> shell
        let client_to_shell_h = s.spawn(|| -> anyhow::Result<()> {
            let mut buf: Vec<u8> = vec![0; BUF_SIZE];
            let mut stdin = inner.shell_proc.stdin.as_ref().ok_or(anyhow!("missing stdin"))?;
            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }


                let len = client_read_stream.read(&mut buf).context("reading client chunk")?;
                
                let mut to_write = &buf[0..len];
                while to_write.len() > 0 {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

                    // We need to select on the stdin fd before actually doing
                    // the write because we can't block this thread without
                    // waking up to check if we are supposed to bail every so
                    // often, and we can't just set a timeout on the stdin
                    // handle unfortunately. This means that shpool only
                    // works on unix systems.
                    let mut stdin_set = nix::sys::select::FdSet::new();
                    stdin_set.insert(stdin.as_raw_fd());
                    let mut poll_dur = PIPE_POLL_DURATION_TIMEVAL.clone();
                    let nready = nix::sys::select::select(
                        None, // nix will auto calculate nfds for us
                        None,
                        Some(&mut stdin_set),
                        None,
                        Some(&mut poll_dur),
                    ).context("selecting on stdin")?;
                    if nready == 0 || !stdin_set.contains(stdin.as_raw_fd()) {
                        // we got a timeout, so it is time to check stop_rx again
                        continue;
                    }

                    let nwritten = stdin.write(&to_write).context("writing client chunk")?;
                    to_write = &to_write[nwritten..];
                }
            }
        });

        // shell -> client
        let shell_to_client_h = s.spawn(|| -> anyhow::Result<()> {
            let mut buf: Vec<u8> = vec![0; BUF_SIZE];
            let stdout = inner.shell_proc.stdout.as_mut()
                    .ok_or(anyhow!("missing stdout"))?;
            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                let mut stdout_set = nix::sys::select::FdSet::new();
                stdout_set.insert(stdout.as_raw_fd());
                let mut poll_dur = PIPE_POLL_DURATION_TIMEVAL.clone();
                let nready = nix::sys::select::select(
                    None,
                    Some(&mut stdout_set),
                    None,
                    None,
                    Some(&mut poll_dur),
                ).context("selecting on stdout")?;
                if nready == 0 || !stdout_set.contains(stdout.as_raw_fd()) {
                    continue;
                }

                let len = stdout.read(&mut buf).context("reading shell chunk")?;

                let mut to_write = &buf[0..len];
                while to_write.len() > 0 {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

                    let nwritten = inner.client_stream
                        .write(&to_write).context("writing shell chunk")?;
                    to_write = &to_write[nwritten..];
                }
            }
        });

        loop {
            if client_to_shell_h.is_finished() || shell_to_client_h.is_finished() {
                stop_tx.send(true).context("sending stop msg")?;
                break;
            }
            thread::sleep(JOIN_POLL_DURATION);
        }
        match client_to_shell_h.join() {
            Ok(v) => v?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }
        match shell_to_client_h.join() {
            Ok(v) => v?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }

        Ok(())
    })?;

    Ok(())
} 

fn spawn_subshell(client_stream: UnixStream) -> anyhow::Result<ShellSessionInner> {
    // TODO(ethan): what about stderr? I need to pass that back to the client
    //              as well. Maybe I should make a little framing protocol and
    //              have some stdin frames as well as stdout frames.
    let child = process::Command::new(user_default_shell()?)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .spawn()
        .context("spawning subshell")?;

    Ok(ShellSessionInner {
        shell_proc: child,
        client_stream,
    })
}

fn user_default_shell() -> anyhow::Result<String> {
    let out = process::Command::new("/bin/sh")
        .arg("-c")
        .arg("echo $SHELL")
        .output()
        .context("spawning subshell to determine default shell")?;
    if !out.status.success() {
        return Err(anyhow!("bad status checking for default shell: {}", out.status));
    }
    if out.stderr.len() != 0 {
        return Err(anyhow!("unexpected stderr when checking for default shell: {}",
                           String::from_utf8_lossy(&out.stderr)));
    }

    Ok(String::from_utf8(out.stdout).context("parsing default shell as utf8")?)
}


fn parse_attach_header(stream: &mut UnixStream) -> anyhow::Result<protocol::AttachHeader> {
    let length_prefix = stream.read_u32::<LittleEndian>()
        .context("reading header length prefix")?;
    let mut buf: Vec<u8> = vec![0; length_prefix as usize];
    stream.read_exact(&mut buf).context("reading header buf")?;

    let header: protocol::AttachHeader =
        rmp_serde::from_slice(&buf).context("parsing header")?;
    Ok(header)
}

fn write_attach_reply_header(
    stream: &mut UnixStream,
    header: protocol::AttachReplyHeader,
) -> anyhow::Result<()> {
    stream.set_write_timeout(Some(SOCK_STREAM_TIMEOUT))
        .context("setting write timout on inbound session")?;

    let buf = rmp_serde::to_vec(&header).context("formatting reply header")?;
    stream.write_u32::<LittleEndian>(buf.len() as u32)
        .context("writing reply length prefix")?;
    stream.write_all(&buf).context("writing reply header")?;

    stream.set_write_timeout(None)
        .context("unsetting write timout on inbound session")?;
    Ok(())
}
