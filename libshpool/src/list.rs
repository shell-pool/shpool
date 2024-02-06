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

use std::{io, path::PathBuf, time};

use anyhow::Context;

use super::{
    protocol,
    protocol::{ConnectHeader, ListReply},
};

pub fn run(socket: PathBuf) -> anyhow::Result<()> {
    let mut client = match protocol::Client::new(socket) {
        Ok(c) => c,
        Err(err) => {
            let io_err = err.downcast::<io::Error>()?;
            if io_err.kind() == io::ErrorKind::NotFound {
                eprintln!("could not connect to daemon");
            }
            return Err(io_err).context("connecting to daemon");
        }
    };

    client.write_connect_header(ConnectHeader::List).context("sending list connect header")?;
    let reply: ListReply = client.read_reply().context("reading reply")?;

    println!("NAME\tSTARTED_AT\tSTATUS");
    for session in reply.sessions.iter() {
        let started_at =
            time::UNIX_EPOCH + time::Duration::from_millis(session.started_at_unix_ms as u64);
        let started_at = chrono::DateTime::<chrono::Utc>::from(started_at);
        println!("{}\t{}\t{}", session.name, started_at.to_rfc3339(), session.status);
    }

    Ok(())
}
