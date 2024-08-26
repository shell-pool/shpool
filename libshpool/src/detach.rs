// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{io, path::Path};

use anyhow::{anyhow, Context};
use shpool_protocol::{ConnectHeader, DetachReply, DetachRequest};

use crate::{common, protocol, protocol::ClientResult};

pub fn run<P>(mut sessions: Vec<String>, socket: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let mut client = match protocol::Client::new(socket) {
        Ok(ClientResult::JustClient(c)) => c,
        Ok(ClientResult::VersionMismatch { warning, client }) => {
            eprintln!("warning: {}, try restarting your daemon", warning);
            client
        }
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                eprintln!("could not connect to daemon");
            }
            return Err(io_err).context("connecting to daemon");
        }
    };

    common::resolve_sessions(&mut sessions, "detach")?;

    client
        .write_connect_header(ConnectHeader::Detach(DetachRequest { sessions }))
        .context("writing detach request header")?;

    let reply: DetachReply = client.read_reply().context("reading reply")?;

    if !reply.not_found_sessions.is_empty() {
        eprintln!("not found: {}", reply.not_found_sessions.join(" "));
        return Err(anyhow!("not found: {}", reply.not_found_sessions.join(" ")));
    }
    if !reply.not_attached_sessions.is_empty() {
        eprintln!("not attached: {}", reply.not_attached_sessions.join(" "));
        return Err(anyhow!("not attached: {}", reply.not_attached_sessions.join(" ")));
    }

    Ok(())
}
