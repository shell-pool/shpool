use std::env;
use std::path::PathBuf;

use anyhow::{Context, anyhow};
use log::info;

use super::protocol;
use super::test_hooks;

pub fn run(name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING ATTACH ============================\n\n");
    test_hooks::emit_event("attach-startup");

    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::Attach(protocol::AttachHeader {
        name: name.clone(),
        term: env::var("TERM").context("resolving local $TERM")?,
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

    client.pipe_bytes()
}
