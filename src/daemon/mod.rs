use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::{thread, fs};

use anyhow::Context;
use log::{info, error};

mod config;
mod ssh_plugin;
mod user;
mod shell;
mod server;

pub fn run(config_file: Option<String>, socket: PathBuf) -> anyhow::Result<()> {
    info!("\n\n======================== STARTING DAEMON ============================\n\n");

    let mut config = config::Config::default();
    if let Some(config_path) = config_file {
        let config_str = fs::read_to_string(config_path).context("reading config toml")?;
        config = toml::from_str(&config_str).context("parsing config file")?;
    }

    let mut server = server::Server::new(config);

    // spawn the signal handler thread in the background
    SignalHandler::new(socket.clone()).spawn()?;

    server.serve(&socket)?;

    std::fs::remove_file(socket).context("cleaning up socket after no more incoming")?;

    Ok(())
}

//
// Signal Handling
//

struct SignalHandler {
    sock: PathBuf,
}
impl SignalHandler {
    fn new(sock: PathBuf) -> Self {
        SignalHandler {
            sock,
        }
    }

    fn spawn(self) -> anyhow::Result<()> {
        use signal_hook::consts::TERM_SIGNALS;
        use signal_hook::iterator::*;
        use signal_hook::flag;

        // This sets us up to shutdown immediately if someone
        // mashes ^C so we don't get stuck attempting a graceful
        // shutdown.
        let term_now = Arc::new(AtomicBool::new(false));
        for sig in TERM_SIGNALS {
            // When terminated by a second term signal, exit with exit code 1.
            // This will do nothing the first time (because term_now is false).
            flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
            // But this will "arm" the above for the second time, by setting it to true.
            // The order of registering these is important, if you put this one first, it will
            // first arm and then terminate ‒ all in the first round.
            flag::register(*sig, Arc::clone(&term_now))?;
        }

        let mut signals = Signals::new(TERM_SIGNALS)
            .context("creating signal iterator")?;

        thread::spawn(move || {
            for signal in &mut signals {
                match signal as libc::c_int {
                    term_sig => {
                        assert!(TERM_SIGNALS.contains(&term_sig));

                        info!("term sig handler : cleaning up socket");
                        if let Err(e)= std::fs::remove_file(self.sock).context("cleaning up socket") {
                            error!("error cleaning up socket file: {}", e);
                        }

                        info!("term sig handler: exiting");
                        std::process::exit(128 + 2 /* default SIGINT exit code */);
                    }
                }
            }
        });

        Ok(())
    }
}
