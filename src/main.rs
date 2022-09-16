use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(version, author, about)]
struct Args {
    #[clap(short, long, action, help = "the file to write logs to")]
    log_file: Option<String>,
    #[clap(short, long, action = clap::ArgAction::Count,
           help = "show more in logs, may be provided multiple times")]
    verbose: u8,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[clap(help = "shpool-daemon starts running a daemon that holds a pool of shells")]
    Daemon {
        #[clap(short, long, action, help = "a toml file containing configuration")]
        config_file: String,
    },
    #[clap(help = "shpool-attach creates or attaches to an existing shell session")]
    Attach {
        #[clap(short, long, action,
               help = "a time, in seconds, to keep the session around after disconnect, infinite if unset")]
        keepalive_secs: Option<u64>,
        #[clap(help = "the name of the shell session to create to attach to")]
        name: String,
    },
    #[clap(help = "shpool-list lists all the running shell sessions")]
    List,
    #[clap(help = r#"shpool-ssh connects to a remote machine with a pool running on it.

All args are passed directly to ssh with the addition of a shpool-attach
command to be run on the remote machine. ssh may be invoked multiple times."#)]
    Ssh {
        #[clap(multiple = true, help = "arguments to pass to the ssh binary")]
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(log_file) = args.log_file.clone() {
        let filter_level = if args.verbose == 0 {
            log::LevelFilter::Info
        } else if args.verbose == 1 {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Trace
        };

        fern::Dispatch::new()
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
            .level_for("shpool", filter_level)
            .chain(fern::log_file(log_file).context("prepping log file")?)
            .apply()?;
    }

    println!("args: {:?}", args);

    Ok(())
}
