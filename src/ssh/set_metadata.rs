use std::{
    io,
    path::PathBuf,
};

use anyhow::Context;
use tracing::info;

use super::super::protocol;

pub fn run(session_name: String, term: String, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n================ STARTING SSH-LOCAL-COMMAND-SET-METADATA =====================\n\n");

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
        .write_connect_header(protocol::ConnectHeader::LocalCommandSetMetadata(
            protocol::SetMetadataRequest {
                name: session_name.clone(),
                term,
            },
        ))
        .context("writing LocalCommandSetMetadata header")?;

    info!("wrote connection header");

    // We don't wait for a reply because ssh will block until the
    // LocalCommand completes before invoking the remote command

    Ok(())
}
