use std::fs;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use anyhow::Context;
use log::info;

mod config;
mod server;
mod shell;
mod signals;
mod ssh_plugin;
mod systemd;
mod user;

pub fn run(config_file: Option<String>, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let mut config = config::Config::default();
    if let Some(config_path) = config_file {
        let config_str = fs::read_to_string(config_path).context("reading config toml")?;
        config = toml::from_str(&config_str).context("parsing config file")?;
    }

    let server = server::Server::new(config);

    let mut cleanup_socket = None;
    let listener = match systemd::activation_socket() {
        Ok(l) => {
            info!("using systemd activation socket");
            l
        },
        Err(e) => {
            info!("no systemd activation socket: {:?}", e);
            cleanup_socket = Some(socket.clone());
            UnixListener::bind(&socket).context("binding to socket")?
        }
    };
    server::Server::serve(server, listener)?;

    // spawn the signal handler thread in the background
    signals::Handler::new(cleanup_socket.clone()).spawn()?;

    if let Some(sock) = cleanup_socket {
        std::fs::remove_file(sock).context(
            "cleaning up socket on exit")?;
    } else {
        info!("systemd manages the socket, so not cleaning it up");
    }

    Ok(())
}
