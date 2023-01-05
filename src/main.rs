use std::path::PathBuf;
use std::env;

use anyhow::Context;
use clap::{Parser, Subcommand};
use log::error;

mod attach;
mod consts;
mod daemon;
mod detach;
mod kill;
mod list;
mod protocol;
mod ssh;
mod tty;

#[macro_use]
mod test_hooks;

#[derive(Parser, Debug)]
#[clap(version, author, about)]
struct Args {
    #[clap(short, long, action,
           long_help = "the file to write logs to

In most modes logs are discarded by default, but if shpool is
running in daemon mode, the logs will go to stderr by default.")]
    log_file: Option<String>,
    #[clap(short, long, action = clap::ArgAction::Count,
           help = "show more in logs, may be provided multiple times")]
    verbose: u8,
    #[clap(short, long, action,
           long_help = "the path for the unix socket to listen on

This defaults to $XDG_RUNTIME_DIR/shpool.socket.

This flag gets overridden by systemd socket activation when
the daemon is launched by systemd.")]
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
    #[clap(about = "make the given session detach from shpool

This does not close the shell. If no session name is provided
$SHPOOL_SESSION_NAME will be used if it is present in the
environment.")]
    Detach {
        #[clap(multiple = true, help = "sessions to detach")]
        sessions: Vec<String>,
    },
    #[clap(about = "kill the given sessions

This detaches the session if it is attached and kills the underlying
shell with a SIGKILL. To more gracefully remove a shell, attach to
it and run 'exit' yourself. If no session name is provided
$SHPOOL_SESSION_NAME will be used if it is present in the
environment.")]
    Kill {
        #[clap(multiple = true, help = "sessions to kill")]
        sessions: Vec<String>,
    },
    #[clap(about = "lists all the running shell sessions")]
    List,
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
    SshLocalCommandSetMetadata {
        session_name: String,
        term: String,
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
        Commands::Detach { sessions } => {
            detach::run(sessions, socket)
        }
        Commands::Kill { sessions } => {
            kill::run(sessions, socket)
        }
        Commands::List => {
            list::run(socket)
        }
        Commands::Plumbing { command } => {
            match command {
                PlumbingCommands::SshRemoteCommand => {
                    ssh::remote_cmd::run(socket)
                }
                PlumbingCommands::SshLocalCommandSetMetadata {
                    session_name, term
                } => {
                    ssh::set_metadata::run(session_name, term, socket)
                }
            }
        }
    };

    if let Err(err) = res {
        error!("shpool: {:?}", err);
        std::process::exit(1);
    }

    Ok(())
}
