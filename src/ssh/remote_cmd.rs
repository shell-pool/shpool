use std::path::PathBuf;

use anyhow::{Context, anyhow};
use log::info;

use super::super::protocol;
// Plan: do the same thing as the attach command, but dial in
//       with the remote command request.

pub fn run(socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n=================== STARTING SSH-REMOTE-COMMAND =======================\n\n");

    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::RemoteCommandLock)
        .context("writing RemoteCommandLock header")?;

    let attach_resp: protocol::AttachReplyHeader = client.read_reply()
        .context("reading attach reply")?;
    match attach_resp.status {
        protocol::AttachStatus::Busy => {
            return Err(anyhow!("BUG: session already has a terminal attached (should be impossible)"))
        }
        protocol::AttachStatus::Attached => {
            info!("attached to an existing session");
        }
        protocol::AttachStatus::Created => {
            info!("created a new session");
        }
        protocol::AttachStatus::Timeout => {
            return Err(anyhow!("timed out waiting for the LocalCommand to give us a name"))
        }
        protocol::AttachStatus::SshExtensionParkingSlotFull => {
            println!("another session is in the process of attaching, please try again");
            return Ok(())
        }
        protocol::AttachStatus::UnexpectedError(err) => {
            return Err(anyhow!("unexpected error: {}", err));
        }
    }

    client.pipe_bytes()
}
