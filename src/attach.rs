use std::os::unix::io::AsRawFd;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use anyhow::Context;
use crossbeam::channel;
use log::info;

use super::consts;
use super::protocol;

pub fn run(name: String, socket: PathBuf) -> anyhow::Result<()> {
    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::Attach(
            protocol::AttachHeader { name: name.clone() }))
        .context("writing attach header")?;

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
    // TODO(ethan): set read/write timeouts on the client

    thread::scope(|s| {
        // stdin -> socket
        let stdin_to_socket_h = s.spawn(|| -> anyhow::Result<()> {
            let mut stdin = std::io::stdin().lock();
            let mut buf = vec![0; consts::BUF_SIZE];

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                let mut stdin_set = nix::sys::select::FdSet::new();
                stdin_set.insert(stdin.as_raw_fd());
                let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                let nready = nix::sys::select::select(
                    None,
                    Some(&mut stdin_set),
                    None,
                    None,
                    Some(&mut poll_dur),
                ).context("selecting on stdout")?;
                if nready == 0 || !stdin_set.contains(stdin.as_raw_fd()) {
                    continue;
                }

                let nread = stdin.read(&mut buf).context("reading chunk from stdin")?;

                let mut to_write = &buf[..nread];
                while to_write.len() > 0 {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

                    let nwritten = write_client_stream.write(to_write).context("writing chunk to server")?;
                    to_write = &to_write[nwritten..];
                }

                write_client_stream.flush().context("flushing client")?;
            }
        });

        // socket -> stdout
        let socket_to_stdout_h = s.spawn(|| -> anyhow::Result<()> {
            let mut stdout = std::io::stdout().lock();
            let mut buf = vec![0; consts::BUF_SIZE];

            loop {
                if let Ok(_) = stop_rx.try_recv() {
                    return Ok(())
                }

                let nread = read_client_stream.read(&mut buf).context("reading ")?;

                let mut to_write = &mut buf[..nread];
                while to_write.len() > 0  {
                    if let Ok(_) = stop_rx.try_recv() {
                        return Ok(())
                    }

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
                    to_write = &mut to_write[nwritten..];
                }

                stdout.flush().context("flushing stdout")?;
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
