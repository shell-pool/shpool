use std::{fs, os::unix::net::UnixListener, path::PathBuf};

use anyhow::Context;
use tracing::{info, instrument};

mod config;
mod exit_notify;
mod keybindings;
mod server;
mod shell;
mod signals;
mod systemd;
mod ttl_reaper;
mod user;

#[instrument(skip_all)]
pub fn run(
    config_file: Option<String>,
    runtime_dir: PathBuf,
    socket: PathBuf,
) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let mut config = config::Config::default();
    if let Some(config_path) = config_file {
        info!("parsing explicitly passed in config ({})", config_path);
        let config_str = fs::read_to_string(config_path).context("reading config toml (1)")?;
        config = toml::from_str(&config_str).context("parsing config file (1)")?;
    } else {
        let user_info = user::info()?;
        let mut config_path = PathBuf::from(user_info.home_dir);
        config_path.push(".config");
        config_path.push("shpool");
        config_path.push("config.toml");
        if config_path.exists() {
            let config_str = fs::read_to_string(config_path).context("reading config toml (2)")?;
            config = toml::from_str(&config_str).context("parsing config file (2)")?;
        }
    }

    let server = server::Server::new(config, runtime_dir);

    let (cleanup_socket, listener) = match systemd::activation_socket() {
        Ok(l) => {
            info!("using systemd activation socket");
            (None, l)
        }
        Err(e) => {
            info!("no systemd activation socket: {:?}", e);
            (Some(socket.clone()), UnixListener::bind(&socket).context("binding to socket")?)
        }
    };
    // spawn the signal handler thread in the background
    signals::Handler::new(cleanup_socket.clone()).spawn()?;

    server::Server::serve(server, listener)?;

    if let Some(sock) = cleanup_socket {
        std::fs::remove_file(sock).context("cleaning up socket on exit")?;
    } else {
        info!("systemd manages the socket, so not cleaning it up");
    }

    Ok(())
}
