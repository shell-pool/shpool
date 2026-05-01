use std::{io, path::PathBuf};

use anyhow::Context;
use shpool_protocol::{ConnectHeader, MaybeSwitch, ModifyVarReply, ModifyVarRequest};

use crate::{protocol, protocol::ClientResult, VarCommands};

pub fn run(socket: PathBuf, command: VarCommands) -> anyhow::Result<()> {
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

    match command {
        VarCommands::List { json } => {
            client.write_connect_header(ConnectHeader::GetVars).context("getting vars")?;
            let maybe_switch: MaybeSwitch = client.read_reply().context("reading reply")?;
            if json {
                let mut obj = serde_json::json!({});
                for (var, val) in maybe_switch.vars.into_iter() {
                    obj[var] = serde_json::Value::String(val);
                }
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                for (var, val) in maybe_switch.vars.into_iter() {
                    println!("{}\t{}", var, val);
                }
            }
        }
        VarCommands::Get { var } => {
            client.write_connect_header(ConnectHeader::GetVars).context("getting vars")?;
            let maybe_switch: MaybeSwitch = client.read_reply().context("reading reply")?;
            for (key, val) in maybe_switch.vars.into_iter() {
                if key == var {
                    println!("{}", val);
                }
            }
        }
        VarCommands::Set { var, val } => {
            client
                .write_connect_header(ConnectHeader::ModifyVar(ModifyVarRequest {
                    var,
                    val: Some(val),
                }))
                .context("setting var")?;
            let _: ModifyVarReply = client.read_reply().context("reading reply")?;
        }
        VarCommands::Unset { var } => {
            client
                .write_connect_header(ConnectHeader::ModifyVar(ModifyVarRequest { var, val: None }))
                .context("setting var")?;
            let _: ModifyVarReply = client.read_reply().context("reading reply")?;
        }
    }

    Ok(())
}
