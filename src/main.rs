use std::path::PathBuf;
use std::env;

use anyhow::Context;
use clap::{Parser, Subcommand};
use log::{info, error};

mod attach;
mod consts;
mod daemon;
mod list;
mod protocol;
mod ssh_local_command_set_name;
mod ssh_remote_command;
mod test_hooks;
mod tty;

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
    #[clap(about = "starts running a daemon that holds a pool of shells")]
    Daemon {
        #[clap(short, long, action, help = "a toml file containing configuration")]
        config_file: Option<String>,
    },
    #[clap(about = "creates or attaches to an existing shell session")]
    Attach {
        #[clap(help = "the name of the shell session to create or attach to")]
        name: String,
    },
    #[clap(about = "lists all the running shell sessions")]
    List,
    #[clap(about = r#"connects to a remote machine with a pool running on it.

All args are passed directly to ssh with the addition of a shpool-attach
command to be run on the remote machine. ssh may be invoked multiple times."#)]
    Ssh {
        #[clap(multiple = true, help = "arguments to pass to the ssh binary")]
        args: Vec<String>,
    },
    #[clap(about = "contains subcommands not meant to be directly invoked")]
    Plumbing{
        #[clap(subcommand)]
        command: PlumbingCommands,
    },
}


#[derive(Subcommand, Debug)]
enum PlumbingCommands {
    #[clap(about = r#"a plumbing command used to extend ssh

See shpool documentation on how to edit your /etc/ssh_config or ~/.ssh/config to take
advantage of this command.
"#)]
    SshRemoteCommand,
    #[clap(about = r#"a plumbing command used to extend ssh

This command is internal to shpool and you should never have to reference it directly, even in your config.
"#)]
    SshLocalCommandSetName {
        session_name: String,
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

    if let Ok(test_hook_sock) = std::env::var("SHPOOL_TEST_HOOK_SOCKET_PATH") {
        info!("spawning test hook sock at {}", test_hook_sock);
        test_hooks::TEST_HOOK_SERVER.set_socket_path(test_hook_sock.clone());
        std::thread::spawn(|| {
            test_hooks::TEST_HOOK_SERVER.start();
        });
        info!("waiting for test hook connection");
        test_hooks::TEST_HOOK_SERVER.wait_for_connect()?;
    }

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
        Commands::Plumbing { command } => {
            match command {
                PlumbingCommands::SshRemoteCommand => {
                    ssh_remote_command::run(socket)
                }
                PlumbingCommands::SshLocalCommandSetName { session_name } => {
                    ssh_local_command_set_name::run(session_name, socket)
                }
            }
        }
    };

    if let Err(err) = res {
        error!("shpool: {:?}", err);
    }

    Ok(())
}
