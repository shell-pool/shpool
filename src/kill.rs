use std::{
    env,
    io,
    path::Path,
};

use anyhow::Context;

use super::protocol;

pub fn run<P>(mut sessions: Vec<String>, socket: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
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

    // if no session has been provided, use the current one
    if sessions.len() == 0 {
        if let Ok(current_session) = env::var("SHPOOL_SESSION_NAME") {
            sessions.push(current_session);
        }
    }

    if sessions.len() == 0 {
        println!("no session to kill");
        std::process::exit(1);
    }

    client
        .write_connect_header(protocol::ConnectHeader::Kill(protocol::KillRequest {
            sessions,
        }))
        .context("writing detach request header")?;

    let reply: protocol::KillReply = client.read_reply().context("reading reply")?;

    let mut exit_status = 0;
    if reply.not_found_sessions.len() > 0 {
        println!("not found: {}", reply.not_found_sessions.join(" "));
        exit_status = 1;
    }

    if exit_status != 0 {
        std::process::exit(exit_status);
    }

    Ok(())
}
