use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;

use anyhow::Context;
use log::{info, error};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::*;
use signal_hook::flag;

pub struct Handler {
    sock: Option<PathBuf>,
}
impl Handler {
    pub fn new(sock: Option<PathBuf>) -> Self {
        Handler {
            sock,
        }
    }

    pub fn spawn(self) -> anyhow::Result<()> {
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

                        info!("term sig handler: cleaning up socket");
                        if let Some(sock) = self.sock {
                            if let Err(e)= std::fs::remove_file(sock)
                                .context("cleaning up socket") {
                                error!("error cleaning up socket file: {}", e);
                            }
                        }

                        info!("term sig handler: exiting");
                        std::process::exit(0);
                    }
                }
            }
        });

        Ok(())
    }
}
