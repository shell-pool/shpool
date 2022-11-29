use std::io::Write;
use std::path::PathBuf;
use std::{process, io};
use std::os::unix::io::AsRawFd;

use anyhow::{anyhow, Context};

use super::{events::Events, line_matcher::LineMatcher};

/// Proc is a handle for a `shpool attach` subprocess
/// spawned for testing
pub struct Proc {
    pub proc: process::Child,
    pub log_file: PathBuf,
    pub events: Option<Events>,
}


impl Proc {
    pub fn run_raw_cmd(
        &mut self,
        mut cmd: Vec<u8>,
    ) -> anyhow::Result<()> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        cmd.push("\n".as_bytes()[0]);
        stdin.write_all(&cmd).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    pub fn run_cmd(
        &mut self,
        cmd: &str,
    ) -> anyhow::Result<()> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        let full_cmd = format!("{}\n", cmd);
        stdin.write_all(full_cmd.as_bytes()).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    /// Create a handle for asserting about output lines.
    ///
    /// For some reason we can't just create the Lines iterator as soon
    /// as we spawn the subcommand. Attempts to do so result in
    /// `Resource temporarily unavailable` (EAGAIN) errors.
    pub fn line_matcher(&mut self) -> anyhow::Result<LineMatcher> {
        let r = self.proc.stdout.take().ok_or(anyhow!("missing stdout"))?;

        nix::fcntl::fcntl(
            r.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        ).context("setting stdin nonblocking")?;

        Ok(LineMatcher{ out: io::BufReader::new(r) })
    }

    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        if let Some(events) = &mut self.events {
            events.await_event(event)
        } else {
            Err(anyhow!("no events stream"))
        }
    }
}

impl std::ops::Drop for Proc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing attach proc: {:?}", e);
        }
    }
}