// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    io,
    os::{fd::BorrowedFd, unix::io::RawFd},
};

use anyhow::Context;
use nix::{
    sys::{
        termios,
        termios::{ControlFlags, InputFlags, LocalFlags, OutputFlags, SetArg},
    },
    unistd::isatty,
};
use shpool_protocol::TtySize;
use tracing::error;

use crate::consts;

// see `man ioctl_tty` for info on these ioctl commands
nix::ioctl_read_bad!(tiocgwinsz, libc::TIOCGWINSZ, libc::winsize);
nix::ioctl_write_ptr_bad!(tiocswinsz, libc::TIOCSWINSZ, libc::winsize);

/// Methods for the TtySize protocol struct. Protocol structs
/// are always bare structs, so we use ext traits to mix in methods.
pub trait TtySizeExt {
    fn from_fd(fd: RawFd) -> anyhow::Result<TtySize>;
    fn set_fd(&self, fd: RawFd) -> anyhow::Result<()>;
}

impl TtySizeExt for TtySize {
    /// from_fd returns the terminal size for the given terminal.
    fn from_fd(fd: RawFd) -> anyhow::Result<TtySize> {
        let mut term_size = libc::winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };

        // Safety: term_size is stack allocated and live for the whole
        //         call.
        unsafe {
            tiocgwinsz(fd, &mut term_size).context("fetching term size")?;
        }

        Ok(TtySize {
            rows: term_size.ws_row,
            cols: term_size.ws_col,
            xpixel: term_size.ws_xpixel,
            ypixel: term_size.ws_ypixel,
        })
    }

    /// set_fd sets the tty indicated by the given file descriptor
    /// to have this size.
    fn set_fd(&self, fd: RawFd) -> anyhow::Result<()> {
        let term_size = libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.xpixel,
            ws_ypixel: self.ypixel,
        };

        // Safety: term_size is live for the whole call.
        unsafe {
            tiocswinsz(fd, &term_size).context("setting term size")?;
        }

        Ok(())
    }
}

pub fn disable_echo(fd: BorrowedFd<'_>) -> anyhow::Result<()> {
    let mut term = termios::tcgetattr(fd).context("grabbing term flags")?;
    term.local_flags &= !LocalFlags::ECHO;

    termios::tcsetattr(fd, SetArg::TCSANOW, &term)?;

    Ok(())
}

pub fn set_attach_flags() -> anyhow::Result<AttachFlagsGuard<'static>> {
    // Safety: stdin is live for the whole program duration
    let fd = unsafe { BorrowedFd::borrow_raw(consts::STDIN_FD) };

    if !isatty(io::stdin())? || !isatty(io::stdout())? || !isatty(io::stderr())? {
        // We are not attached to a terminal, so don't futz with its flags.
        return Ok(AttachFlagsGuard { fd, old: None });
    }

    // grab settings from the stdin terminal
    let old = termios::tcgetattr(fd).context("grabbing term flags")?;

    // Set the input terminal to raw mode so we immediately get the input chars.
    // The terminal for the remote shell is the one that will apply all the logic.
    let mut new = old.clone();
    new.input_flags &= !(InputFlags::IGNBRK
        | InputFlags::BRKINT
        | InputFlags::PARMRK
        | InputFlags::ISTRIP
        | InputFlags::INLCR
        | InputFlags::IGNCR
        | InputFlags::ICRNL
        | InputFlags::IXON);
    new.output_flags &= !OutputFlags::OPOST;
    new.local_flags &= !(LocalFlags::ECHO
        | LocalFlags::ECHONL
        | LocalFlags::ICANON
        | LocalFlags::ISIG
        | LocalFlags::IEXTEN);
    new.control_flags &= !(ControlFlags::CSIZE | ControlFlags::PARENB);
    new.control_flags |= ControlFlags::CS8;
    termios::tcsetattr(fd, SetArg::TCSANOW, &new)?;

    Ok(AttachFlagsGuard { fd, old: Some(old) })
}

pub struct AttachFlagsGuard<'fd> {
    fd: BorrowedFd<'fd>,
    old: Option<termios::Termios>,
}

impl std::ops::Drop for AttachFlagsGuard<'_> {
    fn drop(&mut self) {
        if let Some(old) = &self.old {
            if let Err(e) = termios::tcsetattr(self.fd, SetArg::TCSANOW, old) {
                error!("error restoring terminal settings: {:?}", e);
            }
        }
    }
}
