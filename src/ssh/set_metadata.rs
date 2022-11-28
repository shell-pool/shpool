use std::path::PathBuf;
use std::env;

use anyhow::{anyhow, Context};
use super::super::protocol;

use log::{debug, info};

pub fn run(session_name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n================ STARTING SSH-LOCAL-COMMAND-SET-METADATA =====================\n\n");

    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::LocalCommandSetMetadata(
        protocol::SetMetadataRequest{
            name: session_name.clone(),
            term: env::var("TERM").context("resolving local $TERM")?,
        },
    )).context("writing LocalCommandSetMetadata header")?;

    info!("wrote connection header");

    let reply: protocol::LocalCommandSetMetadataReply = client.read_reply()
        .context("reading LocalCommandSetMetadata reply")?;

    debug!("read reply: {:?}", reply);

    match reply.status {
        protocol::LocalCommandSetMetadataStatus::Timeout => {
            println!("timeout");
            return Err(anyhow!("timeout"));
        }
        protocol::LocalCommandSetMetadataStatus::Ok => {
            info!("set name '{}' for the parked remote command thread", session_name);
        }
    }

    Ok(())
}
