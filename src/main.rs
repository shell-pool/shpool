use std::env;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use log::error;

mod attach;
mod consts;
mod daemon;
mod list;
mod protocol;

#[derive(Parser, Debug)]
#[clap(version, author, about)]
struct Args {
    #[clap(short, long, action, help = "the file to write logs to, by default they are swallowed or go to stderr in daemon mode")]
    log_file: Option<String>,
    #[clap(short, long, action = clap::ArgAction::Count,
           help = "show more in logs, may be provided multiple times")]
    verbose: u8,
    #[clap(short, long, action, help = "the path for the unix socket to listen on, default = $XDG_RUNTIME_DIR/shpool.socket")]
    socket: Option<String>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[clap(about = "shpool-daemon starts running a daemon that holds a pool of shells")]
    Daemon {
        #[clap(short, long, action, help = "a toml file containing configuration")]
        config_file: String,
    },
    #[clap(about = "shpool-attach creates or attaches to an existing shell session")]
    Attach {
        #[clap(help = "the name of the shell session to create or attach to")]
        name: String,
    },
    #[clap(about = "shpool-list lists all the running shell sessions")]
    List,
    #[clap(about = r#"shpool-ssh connects to a remote machine with a pool running on it.

All args are passed directly to ssh with the addition of a shpool-attach
command to be run on the remote machine. ssh may be invoked multiple times."#)]
    Ssh {
        #[clap(multiple = true, help = "arguments to pass to the ssh binary")]
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let filter_level = if args.verbose == 0 {
        log::LevelFilter::Info
    } else if args.verbose == 1 {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Trace
    };

    let mut log_dispatcher = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] {} [{}] {}",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                record.level(),
                record.target(),
                message,
            ));
        })
        .level(log::LevelFilter::Warn)
        .level_for("shpool", filter_level);
    if let Some(log_file) = args.log_file.clone() {
        log_dispatcher = log_dispatcher.chain(
            fern::log_file(log_file).context("prepping log file")?);
    } else if let Commands::Daemon { .. } = args.command {
        log_dispatcher = log_dispatcher.chain(std::io::stderr());
    };
    log_dispatcher.apply().context("creating logger")?;

    let socket = match args.socket {
        Some(s) => PathBuf::from(s),
        None => {
            match env::var("XDG_RUNTIME_DIR").context("getting runtime dir") {
                Ok(runtime_dir) => PathBuf::from(runtime_dir).join("shpool.socket"),
                Err(err) => {
                    error!("{:?}", err);
                    return Ok(());
                }
            }
        },
    };

    let res: anyhow::Result<()> = match args.command {
        Commands::Daemon { config_file } => {
            daemon::run(config_file, socket)
        }
        Commands::Attach { name } => {
            attach::run(name, socket)
        }
        Commands::List => {
            list::run(socket)
        }
        Commands::Ssh { args } => {
            println!("TODO: ssh with args: {:?}", args);
            Ok(())
        }
    };

    if let Err(err) = res {
        error!("{:?}", err);
    }

    Ok(())
}
