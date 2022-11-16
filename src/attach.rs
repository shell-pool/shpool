use std::env;
use std::path::PathBuf;

use anyhow::{Context, anyhow};
use log::{info, warn};

use super::{protocol, test_hooks, tty};

pub fn run(name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING ATTACH ============================\n\n");
    test_hooks::emit_event("attach-startup");

    let mut client = protocol::Client::new(socket)?;

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
