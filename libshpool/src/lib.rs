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

use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io,
    path::PathBuf,
    sync::Mutex,
};

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
pub use hooks::Hooks;
use tracing::error;
use tracing_subscriber::fmt::format::FmtSpan;

mod attach;
mod common;
mod config;
mod consts;
mod daemon;
mod detach;
mod duration;
mod hooks;
mod kill;
mod list;
mod protocol;
mod test_hooks;
mod tty;
mod user;

/// The command line arguments that shpool expects.
/// These can be directly parsed with clap or manually
/// constructed in order to present some other user
/// interface.
///
/// NOTE: You must check `version()` and handle it yourself
/// if it is set. Clap won't do a good job with its
/// automatic version support for a library.
#[derive(Parser, Debug)]
#[clap(author, about)]
pub struct Args {
    #[clap(
        short,
        long,
        action,
        long_help = "The file to write logs to

In most modes logs are discarded by default, but if shpool is
running in daemon mode, the logs will go to stderr by default."
    )]
    pub log_file: Option<String>,

    #[clap(
        short,
        long,
        action = clap::ArgAction::Count,
        help = "Show more in logs, may be provided multiple times",
    )]
    pub verbose: u8,

    #[clap(
        short,
        long,
        action,
        long_help = "The path for the unix socket to listen on

This defaults to $XDG_RUNTIME_DIR/shpool/shpool.socket or ~/.shpool/shpool.socket
if XDG_RUNTIME_DIR is unset.

This flag gets overridden by systemd socket activation when
the daemon is launched by systemd."
    )]
    pub socket: Option<String>,

    #[clap(short, long, action, help = "a toml file containing configuration")]
    pub config_file: Option<String>,

    #[clap(subcommand)]
    pub command: Commands,
}

/// The subcommds that shpool supports.
#[derive(Subcommand, Debug)]
pub enum Commands {
    #[clap(about = "Print version")]
    Version,

    #[clap(about = "Starts running a daemon that holds a pool of shells")]
    Daemon,

    #[clap(about = "Creates or attaches to an existing shell session")]
    Attach {
        #[clap(short, long, help = "If a tty is already attached to the session, detach it first")]
        force: bool,
        #[clap(
            long,
            long_help = "Automatically kill the session after the given time

This option only applies when first creating a session, it is ignored on
reattach.

The duration can be specified either in a colon seperated format
of the form dd:hh:mm:ss where any prefix may be left off (i.e. '01:00:30:00'
for 1 day and 30 minutes or '10:45:00' for 10 hours and 45 minutes), or
using a number with a trailing letter to indicate time unit
(i.e. '3d', '19h', or '5s')."
        )]
        ttl: Option<String>,
        #[clap(
            short,
            long,
            long_help = "A command to run instead of the user's default shell

The command is broken up into a binary to invoke and a list of arguments to
pass to the binary using the shell-words crate."
        )]
        cmd: Option<String>,
        #[clap(help = "The name of the shell session to create or attach to")]
        name: String,
    },

    #[clap(about = "Make the given session detach from shpool

This does not close the shell. If no session name is provided
$SHPOOL_SESSION_NAME will be used if it is present in the
environment.")]
    Detach {
        #[clap(help = "sessions to detach")]
        sessions: Vec<String>,
    },

    #[clap(about = "Kill the given sessions

This detaches the session if it is attached and kills the underlying
shell with a SIGHUP followed by a SIGKILL if the shell fails to exit
quickly enough. If no session name is provided $SHPOOL_SESSION_NAME
will be used if it is present in the environment.")]
    Kill {
        #[clap(help = "sessions to kill")]
        sessions: Vec<String>,
    },

    #[clap(about = "lists all the running shell sessions")]
    List,
}

impl Args {
    /// Version indicates if the wrapping binary must display the
    /// version then exit.
    pub fn version(&self) -> bool {
        matches!(self.command, Commands::Version)
    }
}

/// Run the shpool tool with the given arguments. If hooks is provided,
/// inject the callbacks into the daemon.
pub fn run(args: Args, hooks: Option<Box<dyn hooks::Hooks + Send + Sync>>) -> anyhow::Result<()> {
    match (&args.command, env::var(consts::SENTINEL_FLAG_VAR).as_deref()) {
        (Commands::Daemon, Ok("prompt")) => {
            println!("{}", consts::PROMPT_SENTINEL);
            std::process::exit(0);
        }
        (Commands::Daemon, Ok("startup")) => {
            println!("{}", consts::STARTUP_SENTINEL);
            std::process::exit(0);
        }
        _ => {}
    }

    let trace_level = if args.verbose == 0 {
        tracing::Level::INFO
    } else if args.verbose == 1 {
        tracing::Level::DEBUG
    } else {
        tracing::Level::TRACE
    };
    if let Some(log_file) = args.log_file.clone() {
        let file = fs::File::create(log_file)?;
        tracing_subscriber::fmt()
            .with_max_level(trace_level)
            .with_thread_ids(true)
            .with_target(false)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_writer(Mutex::new(file))
            .init();
    } else if let Commands::Daemon { .. } = args.command {
        tracing_subscriber::fmt()
            .with_max_level(trace_level)
            .with_thread_ids(true)
            .with_target(false)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_writer(io::stderr)
            .init();
    }

    #[cfg(feature = "test_hooks")]
    if let Ok(test_hook_sock) = std::env::var("SHPOOL_TEST_HOOK_SOCKET_PATH") {
        log::info!("spawning test hook sock at {}", test_hook_sock);
        test_hooks::TEST_HOOK_SERVER.set_socket_path(test_hook_sock.clone());
        std::thread::spawn(|| {
            test_hooks::TEST_HOOK_SERVER.start();
        });
        log::info!("waiting for test hook connection");
        test_hooks::TEST_HOOK_SERVER.wait_for_connect()?;
    }

    let mut runtime_dir = match env::var("XDG_RUNTIME_DIR") {
        Ok(runtime_dir) => PathBuf::from(runtime_dir),
        Err(_) => {
            PathBuf::from(env::var("HOME").context("no XDG_RUNTIME_DIR or HOME")?).join(".shpool")
        }
    }
    .join("shpool");

    let socket = match args.socket {
        Some(s) => {
            // The user can reasonably expect that if they provide seperate
            // sockets for differnt shpool instances to run on, they won't
            // stomp on one another. To respect this expectation we need to
            // namespace the rest of the runtime data if they provide a socket
            // name. A short hash is probably good enough.
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let hash = hasher.finish();
            runtime_dir = runtime_dir.join(format!("{:x}", hash));

            PathBuf::from(s)
        }
        None => runtime_dir.join("shpool.socket"),
    };

    let res: anyhow::Result<()> = match args.command {
        Commands::Version => return Err(anyhow!("wrapper binary must handle version")),
        Commands::Daemon => daemon::run(
            args.config_file,
            runtime_dir,
            hooks.unwrap_or(Box::new(NoopHooks {})),
            socket,
        ),
        Commands::Attach { force, ttl, cmd, name } => {
            attach::run(args.config_file, name, force, ttl, cmd, socket)
        }
        Commands::Detach { sessions } => detach::run(sessions, socket),
        Commands::Kill { sessions } => kill::run(sessions, socket),
        Commands::List => list::run(socket),
    };

    if let Err(err) = res {
        error!("{:?}", err);
        std::process::exit(1);
    }

    Ok(())
}

struct NoopHooks {}
impl hooks::Hooks for NoopHooks {}
