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

use std::{path::PathBuf, time};

use anyhow::Context;
use chrono::{DateTime, Utc};
use shpool_protocol::{ConnectHeader, ListReply};

use crate::protocol;

pub fn run(socket: PathBuf, json_output: bool) -> anyhow::Result<()> {
    let mut client = protocol::connect_cli(socket)?;

    client.write_connect_header(ConnectHeader::List).context("sending list connect header")?;
    let reply: ListReply = client.read_reply().context("reading reply")?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&reply)?);
    } else {
        println!("NAME\tSTARTED_AT\tSTATUS");
        for session in reply.sessions.iter() {
            let started_at =
                time::UNIX_EPOCH + time::Duration::from_millis(session.started_at_unix_ms as u64);
            let started_at = DateTime::<Utc>::from(started_at);
            println!("{}\t{}\t{}", session.name, started_at.to_rfc3339(), session.status);
        }
    }

    Ok(())
}
