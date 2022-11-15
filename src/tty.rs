use std::os::unix::io::RawFd;

use anyhow::Context;
use log::error;

pub fn disable_echo(fd: RawFd) -> std::io::Result<()> {
    use termios::*;

    let mut term = Termios::from_fd(fd)?;
    term.c_lflag &= !ECHO;

    tcsetattr(fd, TCSANOW, &term)
}


pub fn set_attach_flags() -> anyhow::Result<AttachFlagsGuard> {
    use termios::*;

    let fd = 0;
    
    if atty::isnt(atty::Stream::Stdout) || atty::isnt(atty::Stream::Stdin) || atty::isnt(atty::Stream::Stderr) {
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
