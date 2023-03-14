use std::{
    fs,
    os::unix::net::UnixListener,
    path::PathBuf,
};

use anyhow::Context;
use tracing::{
    info,
    span,
    Level,
};

mod config;
mod server;
mod shell;
mod signals;
mod systemd;
mod user;

pub fn run(
    config_file: Option<String>,
    runtime_dir: PathBuf,
    socket: PathBuf,
) -> anyhow::Result<()> {
    let _s = span!(Level::INFO, "run").entered();
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let mut config = config::Config::default();
    if let Some(config_path) = config_file {
        let config_str = fs::read_to_string(config_path).context("reading config toml")?;
        config = toml::from_str(&config_str).context("parsing config file")?;
    }

    let server = server::Server::new(config, runtime_dir);

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
        },
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
