use std::{env, io, thread};
use std::path::PathBuf;

use anyhow::{Context, anyhow};
use log::{info, warn, error};

use super::{protocol, test_hooks, tty};

pub fn run(name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING ATTACH ============================\n\n");
    test_hooks::emit!("attach-startup");
    SignalHandler::new(name.clone(), socket.clone()).spawn()?;

    let mut client = match protocol::Client::new(&socket) {
        Ok(c) => c,
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                println!("could not connect to daemon");
            }
            return Err(io_err).context("connecting to daemon");
        }
    };

    let tty_size = match tty::Size::from_fd(0) {
        Ok(s) => s,
        Err(e) => {
            warn!("stdin is not a tty, using default size (err: {:?})", e);
            tty::Size { rows: 24, cols: 80 }
        }
    };

    client.write_connect_header(protocol::ConnectHeader::Attach(protocol::AttachHeader {
        name: name.clone(),
        term: env::var("TERM").context("resolving local $TERM")?,
        local_tty_size: tty_size,
        local_env: env::vars().collect::<Vec<(String, String)>>(),
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
        protocol::AttachStatus::Timeout => {
            return Err(anyhow!("BUG: unexpected timeout (should be impossible)"))
        }
        protocol::AttachStatus::SshExtensionParkingSlotFull => {
            return Err(anyhow!("BUG: unexpected parking lot full status (should be impossible)"))
        }
        protocol::AttachStatus::UnexpectedError(err) => {
            return Err(anyhow!("BUG: unexpected error attaching to '{}': {}", name, err))
        }
    }

    let _tty_guard = tty::set_attach_flags();

    client.pipe_bytes()
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
        SignalHandler {
            session_name,
            socket,
        }
    }

    fn spawn(self) -> anyhow::Result<()> {
        use signal_hook::consts::*;
        use signal_hook::iterator::*;

        let sigs = vec![
            SIGWINCH
        ];
        let mut signals = Signals::new(&sigs)
            .context("creating signal iterator")?;

        thread::spawn(move || {
            for signal in &mut signals {
                let res = match signal as libc::c_int {
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
        client.write_connect_header(protocol::ConnectHeader::SessionMessage(protocol::SessionMessageRequest {
            session_name: self.session_name.clone(),
            payload: protocol::SessionMessageRequestPayload::Resize(protocol::ResizeRequest {
                tty_size: tty_size.clone(),
            }),
        })).context("writing resize request")?;

        let reply: protocol::SessionMessageReply = client.read_reply()
            .context("reading session message reply")?;
        match reply {
            protocol::SessionMessageReply::NotFound => {
                warn!("handle_sigwinch: sent resize for session '{}', but the daemon has no record of that session", self.session_name);
            }
            protocol::SessionMessageReply::Resize(protocol::ResizeReply::Ok) => {
                info!("handle_sigwinch: resized session '{}' to {:?}",
                      self.session_name, tty_size);
            }
            reply => {
                warn!("handle_sigwinch: unexpected resize reply: {:?}", reply);
            }
        }

        Ok(())
    }
}
