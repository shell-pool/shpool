// Copyright 2026 Google LLC
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
use shpool_protocol::{ConnectHeader, RenameReply, RenameRequest};

use crate::{common, protocol, protocol::ClientResult};

pub fn run<P>(mut names: Vec<String>, socket: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let new_name = names.pop().ok_or(anyhow!("no new session name"))?;
    let old_name = match names.pop() {
        Some(name) => name,
        None => match common::current_session_name(&socket) {
            Some(name) => name,
            None => {
                eprintln!("no session to rename");
                return Err(anyhow!("no session to rename"));
            }
        },
    };

    let mut client = match protocol::Client::new(&socket) {
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
        .write_connect_header(ConnectHeader::Rename(RenameRequest {
            old_name: old_name.clone(),
            new_name: new_name.clone(),
        }))
        .context("writing rename request header")?;

    let reply: RenameReply = client.read_reply().context("reading reply")?;
    let err = match reply {
        RenameReply::Ok => return Ok(()),
        RenameReply::NotFound => format!("not found: {old_name}"),
        RenameReply::AlreadyExists => format!("already exists: {new_name}"),
        RenameReply::InvalidName(msg) => msg,
    };
    eprintln!("{err}");
    Err(anyhow!(err))
}
