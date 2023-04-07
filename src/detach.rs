use std::{
    io,
    path::Path,
};

use anyhow::{
    anyhow,
    Context,
};

use super::{
    common,
    protocol,
    protocol::{
        ConnectHeader,
        DetachReply,
        DetachRequest,
    },
};

pub fn run<P>(mut sessions: Vec<String>, socket: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let mut client = match protocol::Client::new(socket) {
        Ok(c) => c,
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                eprintln!("could not connect to daemon");
            }
            return Err(io_err).context("connecting to daemon");
        },
    };

    common::resolve_sessions(&mut sessions, "detach")?;

    client
        .write_connect_header(ConnectHeader::Detach(DetachRequest { sessions }))
        .context("writing detach request header")?;

    let reply: DetachReply = client.read_reply().context("reading reply")?;

    if reply.not_found_sessions.len() > 0 {
        eprintln!("not found: {}", reply.not_found_sessions.join(" "));
        return Err(anyhow!("not found: {}", reply.not_found_sessions.join(" ")));
    }
    if reply.not_attached_sessions.len() > 0 {
        eprintln!("not attached: {}", reply.not_attached_sessions.join(" "));
        return Err(anyhow!(
            "not attached: {}",
            reply.not_attached_sessions.join(" ")
        ));
    }

    Ok(())
}
