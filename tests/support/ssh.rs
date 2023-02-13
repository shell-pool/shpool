use std::{
    io,
    io::Write,
    os::unix::io::AsRawFd,
    process,
};

use anyhow::{
    anyhow,
    Context,
};

use super::{
    events::Events,
    line_matcher::LineMatcher,
};

pub struct RemoteCmdProc {
    pub proc: process::Child,
    pub events: Option<Events>,
}

impl RemoteCmdProc {
    pub fn run_cmd(&mut self, cmd: &str) -> anyhow::Result<()> {
        let stdin = self.proc.stdin.as_mut().ok_or(anyhow!("missing stdin"))?;

        let full_cmd = format!("{}\n", cmd);
        stdin
            .write_all(full_cmd.as_bytes())
            .context("writing cmd into remote cmd proc")?;
        stdin.flush().context("flushing cmd")?;

        Ok(())
    }

    pub fn line_matcher(&mut self) -> anyhow::Result<LineMatcher> {
        let r = self.proc.stdout.take().ok_or(anyhow!("missing stdout"))?;

        nix::fcntl::fcntl(
            r.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .context("setting stdin nonblocking")?;

        Ok(LineMatcher {
            out: io::BufReader::new(r),
        })
    }
}

impl std::ops::Drop for RemoteCmdProc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing remote cmd proc: {:?}", e);
        }
    }
}

pub struct SetMetadataProc {
    pub proc: process::Child,
    pub events: Option<Events>,
}

impl SetMetadataProc {
    pub fn line_matcher(&mut self) -> anyhow::Result<LineMatcher> {
        let r = self.proc.stdout.take().ok_or(anyhow!("missing stdout"))?;

        nix::fcntl::fcntl(
            r.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .context("setting stdin nonblocking")?;

        Ok(LineMatcher {
            out: io::BufReader::new(r),
        })
    }
}

impl std::ops::Drop for SetMetadataProc {
    fn drop(&mut self) {
        if let Err(e) = self.proc.kill() {
            eprintln!("err killing set metadata proc: {:?}", e);
        }
    }
}
