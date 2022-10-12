use std::path::PathBuf;

use anyhow::{anyhow, Context};
use super::protocol;

use log::info;

pub fn run(session_name: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n================ STARTING SSH-LOCAL-COMMAND-SET-NAME =====================\n\n");

    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::LocalCommandSetName(
        protocol::LocalCommandSetNameRequest{
            name: session_name.clone(),
        },
    )).context("writing LocalCommandSetName header")?;

    let reply: protocol::LocalCommandSetNameReply = client.read_reply()
        .context("reading LocalCommandSetName reply")?;
    match reply.status {
        protocol::LocalCommandSetNameStatus::Timeout => {
            return Err(anyhow!("timeout"));
        }
        protocol::LocalCommandSetNameStatus::Ok => {
            info!("set name '{}' for the parked remote command thread", session_name);
        }
    }

    Ok(())
}
