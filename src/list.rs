use std::path::PathBuf;
use std::time;

use anyhow::Context;
use anyhow;

use super::protocol;

pub fn run(socket: PathBuf) -> anyhow::Result<()> {
    let mut client = protocol::Client::new(socket)?;

    client.write_connect_header(protocol::ConnectHeader::List).context("sending list connect header")?;
    let reply: protocol::ListReply = client.read_reply().context("reading reply")?;

    println!("NAME\tSTARTED_AT");
    for session in reply.sessions.iter() {
        let started_at = time::UNIX_EPOCH + time::Duration::from_millis(session.started_at_unix_ms as u64);
        let started_at = chrono::DateTime::<chrono::Utc>::from(started_at);
        println!("{}\t{}", session.name, started_at.to_rfc3339());
    }

    Ok(())
}
