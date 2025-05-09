// Copyright 2025 Google LLC
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

use std::{io, path::PathBuf};

use anyhow::Context;
use shpool_protocol::{ConnectHeader, LogLevel, SetLogLevelReply, SetLogLevelRequest};

use crate::{protocol, protocol::ClientResult};

pub fn run(level: LogLevel, socket: PathBuf) -> anyhow::Result<()> {
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

    client
        .write_connect_header(ConnectHeader::SetLogLevel(SetLogLevelRequest { level }))
        .context("sending set-log-level header")?;
    let _reply: SetLogLevelReply = client.read_reply().context("reading reply")?;

    Ok(())
}
