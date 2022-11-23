use std::path::PathBuf;
use std::fs;

use anyhow::Context;
use log::info;

mod config;
mod ssh_plugin;
mod user;
mod shell;
mod server;
mod signals;

pub fn run(config_file: Option<String>, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let mut config = config::Config::default();
    if let Some(config_path) = config_file {
        let config_str = fs::read_to_string(config_path).context("reading config toml")?;
        config = toml::from_str(&config_str).context("parsing config file")?;
    }

    let mut server = server::Server::new(config);

    // spawn the signal handler thread in the background
    signals::Handler::new(socket.clone()).spawn()?;

    server.serve(&socket)?;

    std::fs::remove_file(socket).context("cleaning up socket after no more incoming")?;

    Ok(())
}
