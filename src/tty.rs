use std::os::unix::io::RawFd;

use anyhow::Context;
use log::error;
use serde_derive::{
    Deserialize,
    Serialize,
};

// see `man ioctl_tty` for info on these ioctl commands
nix::ioctl_read_bad!(tiocgwinsz, libc::TIOCGWINSZ, libc::winsize);
nix::ioctl_write_ptr_bad!(tiocswinsz, libc::TIOCSWINSZ, libc::winsize);

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}
impl Size {
    /// from_fd returns the terminal size for the given terminal.
    pub fn from_fd(fd: RawFd) -> anyhow::Result<Size> {
        let mut term_size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // Saftey: term_size is stack allocated and live for the whole
        //         call.
        unsafe {
            tiocgwinsz(fd, &mut term_size).context("fetching term size")?;
        }

        Ok(Size {
            rows: term_size.ws_row,
            cols: term_size.ws_col,
        })
    }

    /// set_fd sets the tty indicated by the given file descriptor
    /// to have this size.
    pub fn set_fd(&self, fd: RawFd) -> anyhow::Result<()> {
        let term_size = libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            tiocswinsz(fd, &term_size).context("setting term size")?;
        }

        Ok(())
    }
}

pub fn disable_echo(fd: RawFd) -> std::io::Result<()> {
    use termios::*;

    let mut term = Termios::from_fd(fd)?;
    term.c_lflag &= !ECHO;

    tcsetattr(fd, TCSANOW, &term)
}

pub fn set_attach_flags() -> anyhow::Result<AttachFlagsGuard> {
    // TODO(ethan): it seems like we may be able to drop the termios
    //              dep and just use nix. See nix::sys::termios::Termios.
    use termios::*;

    let fd = 0;

    if atty::isnt(atty::Stream::Stdout)
        || atty::isnt(atty::Stream::Stdin)
        || atty::isnt(atty::Stream::Stderr)
    {
        // We are not attached to a terminal, so don't futz with its flags.
        return Ok(AttachFlagsGuard { fd, old: None });
    }

    // grab settings from the stdin terminal
    let old = Termios::from_fd(fd).context("grabbing term flags")?;

    // Set the input terminal to raw mode so we immediately get the input chars.
    // The terminal for the remote shell is the one that will apply all the logic.
    let mut new = old.clone();
    new.c_iflag &= !(IGNBRK | BRKINT | PARMRK | ISTRIP | INLCR | IGNCR | ICRNL | IXON);
    new.c_oflag &= !OPOST;
    new.c_lflag &= !(ECHO | ECHONL | ICANON | ISIG | IEXTEN);
    new.c_cflag &= !(CSIZE | PARENB);
    new.c_cflag |= CS8;
    tcsetattr(fd, TCSANOW, &new)?;

    Ok(AttachFlagsGuard { fd, old: Some(old) })
}

pub struct AttachFlagsGuard {
    fd: RawFd,
    old: Option<termios::Termios>,
}
impl std::ops::Drop for AttachFlagsGuard {
    fn drop(&mut self) {
        if let Some(old) = self.old {
            if let Err(e) = termios::tcsetattr(self.fd, termios::TCSANOW, &old) {
                error!("error restoring terminal settings: {:?}", e);
            }
        }
    }
}
