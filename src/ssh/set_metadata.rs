use std::path::PathBuf;
use std::env;

use anyhow::{anyhow, Context};
use super::super::protocol;

use log::{info, warn};

use super::super::tty;

pub fn run(session_name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n================ STARTING SSH-LOCAL-COMMAND-SET-NAME =====================\n\n");

    let mut client = protocol::Client::new(socket)?;

    let tty_size = match tty::Size::from_fd(0) {
        Ok(s) => s,
        Err(e) => {
            warn!("stdin is not a tty, using default size (err: {:?})", e);
            tty::Size { rows: 24, cols: 80 }
        }
    };

    client.write_connect_header(protocol::ConnectHeader::LocalCommandSetMetadata(
        protocol::SetMetadataRequest{
            name: session_name.clone(),
            term: env::var("TERM").context("resolving local $TERM")?,
            local_tty_size: tty_size,
        },
    )).context("writing LocalCommandSetMetadata header")?;

    let reply: protocol::LocalCommandSetMetadataReply = client.read_reply()
        .context("reading LocalCommandSetMetadata reply")?;
    match reply.status {
        protocol::LocalCommandSetMetadataStatus::Timeout => {
            return Err(anyhow!("timeout"));
        }
        protocol::LocalCommandSetMetadataStatus::Ok => {
            info!("set name '{}' for the parked remote command thread", session_name);
        }
    }

    Ok(())
}
