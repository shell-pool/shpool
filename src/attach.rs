use std::os::unix::io::AsRawFd;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use anyhow::Context;
use crossbeam::channel;
use log::{info, debug, trace};

use super::consts;
use super::protocol;

pub fn run(name: String, socket: PathBuf) -> anyhow::Result<()> {
    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::Attach(protocol::AttachHeader {
        name: name.clone(),
    })).context("writing attach header")?;

    let attach_resp: protocol::AttachReplyHeader = client.read_reply()
        .context("reading attach reply")?;
    match attach_resp.status {
        protocol::AttachStatus::Busy => {
            println!("session '{}' already has a terminal attached", name);
            return Ok(())
        }
        protocol::AttachStatus::Attached => {
            info!("attached to an existing session: '{}'", name);
        }
        protocol::AttachStatus::Created => {
            info!("created a new session: '{}'", name);
        }
    }

    pipe_bytes(client)
}

fn pipe_bytes(client: protocol::Client) -> anyhow::Result<()> {
    let (stop_tx, stop_rx) = channel::bounded(1);

    let mut read_client_stream = client.stream.try_clone().context("cloning read stream")?;
    let mut write_client_stream = client.stream.try_clone().context("cloning read stream")?;

    thread::scope(|s| {
        // stdin -> socket
        let stdin_to_socket_h = s.spawn(|| -> anyhow::Result<()> {
            info!("pipe_bytes: stdin->socket thread spawned");

            let mut stdin = std::io::stdin().lock();
            let mut buf = vec![0; consts::BUF_SIZE];

            nix::fcntl::fcntl(
                stdin.as_raw_fd(),
                nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
            ).context("setting stdin nonblocking")?;

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                let nread = match stdin.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            trace!("pipe_bytes: stdin->socket: read: WouldBlock");
                            thread::sleep(consts::PIPE_POLL_DURATION);
                            continue;
                        }
                        return Err(e).context("reading stdin from user");
                    }
                };

                debug!("pipe_bytes: stdin->socket: read {} bytes", nread);

                let mut to_write = &buf[..nread];
                trace!("pipe_bytes: stdin->socket: created to_write={:?}", to_write);
                while to_write.len() > 0 {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

                    let nwritten = write_client_stream.write(to_write).context("writing chunk to server")?;
                    to_write = &to_write[nwritten..];
                    trace!("pipe_bytes: stdin->socket: to_write={:?}", to_write);
                }

                write_client_stream.flush().context("flushing client")?;
            }
        });

        // socket -> std{out,err}
        let socket_to_stdout_h = s.spawn(|| -> anyhow::Result<()> {
            info!("pipe_bytes: socket->std{{out,err}} thread spawned");

            let mut stdout = std::io::stdout().lock();
            let mut stderr = std::io::stderr().lock();
            let mut buf = vec![0; consts::BUF_SIZE];

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                let chunk = protocol::Chunk::read_into(&mut read_client_stream, &mut buf)
                    .context("reading output chunk from daemon")?;

                debug!("pipe_bytes: socket->std{{out,err}}: read chunk kind={:?} len={}",
                   chunk.kind, chunk.buf.len());
                trace!("pipe_bytes: socket->std{{out,err}}: chunk={:?}", chunk);

                let mut to_write = &chunk.buf[..];
                match chunk.kind {
                    protocol::ChunkKind::NoOp => {
                        trace!("pipe_bytes: got noop chunk");
                    },
                    protocol::ChunkKind::Stdout => {
                        while to_write.len() > 0  {
                            if let Ok(_) = stop_rx.try_recv() {
                                return Ok(())
                            }

                            debug!("pipe_bytes: socket->std{{out,err}}: about to select on stdout");
                            let mut stdout_set = nix::sys::select::FdSet::new();
                            stdout_set.insert(stdout.as_raw_fd());
                            let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                            let nready = nix::sys::select::select(
                                None,
                                None,
                                Some(&mut stdout_set),
                                None,
                                Some(&mut poll_dur),
                            ).context("selecting on stdout")?;
                            if nready == 0 || !stdout_set.contains(stdout.as_raw_fd()) {
                                continue;
                            }

                            let nwritten = stdout.write(to_write).context("writing chunk to stdout")?;
                            debug!("pipe_bytes: socket->std{{out,err}}: wrote {} stdout bytes",
                                nwritten);
                            to_write = &to_write[nwritten..];
                        }

                        stdout.flush().context("flushing stdout")?;
                        debug!("pipe_bytes: socket->std{{out,err}}: flushed stderr");
                    },
                    protocol::ChunkKind::Stderr => {
                        while to_write.len() > 0  {
                            if let Ok(_) = stop_rx.try_recv() {
                                return Ok(())
                            }

                            debug!("pipe_bytes: socket->std{{out,err}}: about to select on stderr");
                            let mut stderr_set = nix::sys::select::FdSet::new();
                            stderr_set.insert(stderr.as_raw_fd());
                            let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                            let nready = nix::sys::select::select(
                                None,
                                None,
                                Some(&mut stderr_set),
                                None,
                                Some(&mut poll_dur),
                            ).context("selecting on stdout")?;
                            if nready == 0 || !stderr_set.contains(stderr.as_raw_fd()) {
                                continue;
                            }

                            let nwritten = stderr.write(to_write).context("writing chunk to stdout")?;
                            debug!("pipe_bytes: socket->std{{out,err}}: wrote {} stderr bytes",
                                nwritten);
                            to_write = &to_write[nwritten..];
                        }

                        stderr.flush().context("flushing stdout")?;
                        debug!("pipe_bytes: socket->std{{out,err}}: flushed stderr");
                    },
                }
            }
        });

        loop {
            if stdin_to_socket_h.is_finished() || socket_to_stdout_h.is_finished() {
                stop_tx.send(true).context("sending stop msg")?;
                break;
            }
            thread::sleep(consts::JOIN_POLL_DURATION);
        }
        match stdin_to_socket_h.join() {
            Ok(v) => v?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }
        match socket_to_stdout_h.join() {
            Ok(v) => v?,
            Err(panic_err) => std::panic::resume_unwind(panic_err),
        }


        Ok(())
    })
}
