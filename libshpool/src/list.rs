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
use chrono::{DateTime, Utc};
use shpool_protocol::{ConnectHeader, ListReply};

use crate::{protocol, protocol::ClientResult};

pub fn run(
    socket: PathBuf,
    show_connected_at: bool,
    show_disconnected_at: bool,
    date_format: Option<String>,
) -> anyhow::Result<()> {
    let mut client = match protocol::Client::new(socket) {
        Ok(ClientResult::JustClient(c)) => c,
        Ok(ClientResult::VersionMismatch { warning, client }) => {
            eprintln!("warning: {warning}, try restarting your daemon");
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

    client.write_connect_header(ConnectHeader::List).context("sending list connect header")?;
    let reply: ListReply = client.read_reply().context("reading reply")?;

    // Helper to format a timestamp
    let format_ts = |ts: DateTime<Utc>| -> String {
        match &date_format {
            Some(fmt) => ts.format(fmt).to_string(),
            None => ts.to_rfc3339(),
        }
    };

    // Helper to format an optional timestamp
    let format_opt_ts = |unix_ms: Option<i64>| -> String {
        match unix_ms {
            Some(ms) => {
                let ts = time::UNIX_EPOCH + time::Duration::from_millis(ms as u64);
                let ts = DateTime::<Utc>::from(ts);
                format_ts(ts)
            }
            None => String::from("-"),
        }
    };

    // Build header
    let mut header = String::from("NAME\tSTARTED_AT");
    if show_connected_at {
        header.push_str("\tCONNECTED_AT");
    }
    if show_disconnected_at {
        header.push_str("\tDISCONNECTED_AT");
    }
    header.push_str("\tSTATUS");
    println!("{}", header);

    for session in reply.sessions.iter() {
        let started_at =
            time::UNIX_EPOCH + time::Duration::from_millis(session.started_at_unix_ms as u64);
        let started_at = DateTime::<Utc>::from(started_at);

        let mut line = format!("{}\t{}", session.name, format_ts(started_at));
        if show_connected_at {
            line.push('\t');
            line.push_str(&format_opt_ts(session.connected_at_unix_ms));
        }
        if show_disconnected_at {
            line.push('\t');
            line.push_str(&format_opt_ts(session.disconnected_at_unix_ms));
        }
        line.push('\t');
        line.push_str(&session.status.to_string());
        println!("{}", line);
    }

    Ok(())
}
