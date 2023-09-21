use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::Context;
use signal_hook::{consts::TERM_SIGNALS, flag, iterator::Signals};
use tracing::{error, info};

pub struct Handler {
    sock: Option<PathBuf>,
}
impl Handler {
    pub fn new(sock: Option<PathBuf>) -> Self {
        Handler { sock }
    }

    pub fn spawn(self) -> anyhow::Result<()> {
        info!("spawning signal handler thread");

        // This sets us up to shutdown immediately if someone
        // mashes ^C so we don't get stuck attempting a graceful
        // shutdown.
        let term_now = Arc::new(AtomicBool::new(false));
        for sig in TERM_SIGNALS {
            // When terminated by a second term signal, exit with exit code 1.
            // This will do nothing the first time (because term_now is false).
            flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
            // But this will "arm" the above for the second time, by setting it to true.
            // The order of registering these is important, if you put this one first, it
            // will first arm and then terminate ‒ all in the first round.
            flag::register(*sig, Arc::clone(&term_now))?;
        }

        let mut signals = Signals::new(TERM_SIGNALS).context("creating signal iterator")?;
        thread::spawn(move || {
            for signal in &mut signals {
                assert!(TERM_SIGNALS.contains(&signal));

                info!("term sig handler: cleaning up socket");
                if let Some(sock) = self.sock {
                    if let Err(e) = std::fs::remove_file(sock).context("cleaning up socket") {
                        error!("error cleaning up socket file: {}", e);
                    }
                }

                info!("term sig handler: exiting");
                std::process::exit(0);
            }
        });

        Ok(())
    }
}
