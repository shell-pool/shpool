use std::{io, io::Write, path::PathBuf, process};

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
    pub fn run_raw(&mut self, cmd: Vec<u8>) -> anyhow::Result<()> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        stdin.write_all(&cmd).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    pub fn run_raw_cmd(&mut self, mut cmd: Vec<u8>) -> anyhow::Result<()> {
        cmd.push("\n".as_bytes()[0]);
        self.run_raw(cmd)
    }

    pub fn run_cmd(&mut self, cmd: &str) -> anyhow::Result<()> {
        eprintln!("running cmd '{cmd}'");
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        let full_cmd = format!("{cmd}\n");
        stdin.write_all(full_cmd.as_bytes()).context("writing cmd into attach proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    /// Create a handle for asserting about stdout output lines.
    ///
    /// For some reason we can't just create the Lines iterator as soon
    /// as we spawn the subcommand. Attempts to do so result in
    /// `Resource temporarily unavailable` (EAGAIN) errors.
    pub fn line_matcher(&mut self) -> anyhow::Result<LineMatcher<process::ChildStdout>> {
        let r = self.proc.stdout.take().ok_or(anyhow!("missing stdout"))?;

        nix::fcntl::fcntl(&r, nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK))
            .context("setting stdout nonblocking")?;

        Ok(LineMatcher { out: io::BufReader::new(r), never_match_regex: vec![] })
    }

    /// Create a handle for asserting about stderr output lines.
    pub fn stderr_line_matcher(&mut self) -> anyhow::Result<LineMatcher<process::ChildStderr>> {
        let r = self.proc.stderr.take().ok_or(anyhow!("missing stderr"))?;

        nix::fcntl::fcntl(&r, nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK))
            .context("setting stderr nonblocking")?;

        Ok(LineMatcher { out: io::BufReader::new(r), never_match_regex: vec![] })
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
            eprintln!("err killing attach proc: {e:?}");
        }
    }
}
