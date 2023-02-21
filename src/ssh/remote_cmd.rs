use std::{
    io,
    path::PathBuf,
};

use anyhow::{
    anyhow,
    Context,
};
use tracing::{
    debug,
    info,
};

use super::super::protocol;

pub fn run(socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n=================== STARTING SSH-REMOTE-COMMAND =======================\n\n");

    let mut client = match protocol::Client::new(socket) {
        Ok(c) => c,
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                println!("could not connect to daemon");
            }
            return Err(io_err).context("connecting to daemon");
        },
    };

    client
        .write_connect_header(protocol::ConnectHeader::RemoteCommandLock)
        .context("writing RemoteCommandLock header")?;

    info!("wrote connect header");

    let attach_resp: protocol::AttachReplyHeader =
        client.read_reply().context("reading attach reply")?;

    debug!("read attach reply: {:?}", attach_resp);

    match attach_resp.status {
        protocol::AttachStatus::Busy => {
            return Err(anyhow!(
                "BUG: session already has a terminal attached (should be impossible)"
            ))
        },
        protocol::AttachStatus::Attached => {
            info!("attached to an existing session");
        },
        protocol::AttachStatus::Created => {
            info!("created a new session");
        },
        protocol::AttachStatus::Timeout => {
            println!("timeout");
            return Err(anyhow!(
                "timed out waiting for the LocalCommand to give us metadata"
            ));
        },
        protocol::AttachStatus::SshExtensionParkingSlotFull => {
            println!("another session is in the process of attaching, please try again");
            return Ok(());
        },
        protocol::AttachStatus::UnexpectedError(err) => {
            return Err(anyhow!("unexpected error: {}", err));
        },
    }

    client.pipe_bytes()
}
