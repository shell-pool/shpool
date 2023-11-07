mod pty;
mod err;

use ::descriptor::Descriptor;

use ::libc;
pub use self::err::{ForkError, Result};
pub use self::pty::{Master, MasterError};
pub use self::pty::{Slave, SlaveError};
use std::ffi::CString;

const MAX_PTS_NAME: usize = 1024;

#[derive(Debug, Clone)]
pub enum Fork {
    // Parent child's pid and master's pty.
    Parent(libc::pid_t, Master),
    // Child pid 0.
    Child(Slave),
}

impl Fork {
    /// The constructor function `new` forks the program
    /// and returns the current pid.
    pub fn new(path: &'static str) -> Result<Self> {
        match Master::new(CString::new(path).ok().unwrap_or_default().as_ptr()) {
            Err(cause) => Err(ForkError::BadMaster(cause)),
            Ok(master) => {
                if let Some(cause) = master.grantpt().err().or(master.unlockpt().err()) {
                    Err(ForkError::BadMaster(cause))
                } else {
                    // Safety: no params to worry about, just an ffi call
                    let fork_ret = unsafe { libc::fork() };
                    match fork_ret {
                        -1 => Err(ForkError::Failure),
                        0 => {
                            let mut ptsname_buf = vec![0; MAX_PTS_NAME];
                            if let Err(cause) = master.ptsname_r(&mut ptsname_buf) {
                                return Err(ForkError::BadMaster(cause));
                            }
                            // ensure null termination
                            let last_idx = ptsname_buf.len() - 1;
                            ptsname_buf[last_idx] = 0;

                            let name: *const u8 = &ptsname_buf[0];
                            Fork::from_pts(name as *const libc::c_char)
                        }
                        pid => Ok(Fork::Parent(pid, master)),
                    }
                }
            },
        }
    }

    /// The constructor function `from_pts` is a private
    /// extention from the constructor function `new` who
    /// prepares and returns the child.
    fn from_pts(ptsname: *const ::libc::c_char) -> Result<Self> {
        if ptsname.is_null() {
            return Err(ForkError::BadPtsname);
        }

        unsafe {
            if libc::setsid() == -1 {
                Err(ForkError::SetsidFail)
            } else {
                match Slave::new(ptsname) {
                    Err(cause) => Err(ForkError::BadSlave(cause)),
                    Ok(slave) => {
                        if let Some(cause) = slave.dup2(libc::STDIN_FILENO)
                            .err()
                            .or(slave.dup2(libc::STDOUT_FILENO)
                                .err()
                                .or(slave.dup2(libc::STDERR_FILENO).err())) {
                            Err(ForkError::BadSlave(cause))
                        } else {
                            Ok(Fork::Child(slave))
                        }
                    }
                }
            }
        }
    }

    /// The constructor function `from_ptmx` forks the program
    /// and returns the current pid for a default PTMX's path.
    pub fn from_ptmx() -> Result<Self> {
        Fork::new(::DEFAULT_PTMX)
    }

    /// Waits until it's terminated.
    pub fn wait(&self) -> Result<libc::pid_t> {
        self.wait_for_exit().map(|(p, _)| p)
    }

    /// Waits until it's terminated, returning the exit status if there is one
    pub fn wait_for_exit(&self) -> Result<(libc::pid_t, Option<i32>)> {
        match *self {
            Fork::Child(_) => Err(ForkError::IsChild),
            Fork::Parent(pid, _) => {
                loop {
                    unsafe {
                        let mut status = 0;
                        match libc::waitpid(pid, &mut status, 0) {
                            0 => continue,
                            -1 => return Err(ForkError::WaitpidFail),
                            _ => {
                                if libc::WIFEXITED(status) {
                                    return Ok((pid, Some(libc::WEXITSTATUS(status))));
                                } else {
                                    return Ok((pid, None));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The function `child_pid` returns the pid of the child process if
    /// this instance of Fork represents the parent process and None
    /// in the child process.
    pub fn child_pid(&self) -> Option<libc::pid_t> {
        match *self {
            Fork::Child(_) => None,
            Fork::Parent(pid, _) => Some(pid),
        }
    }

    /// The function `is_parent` returns the pid or parent
    /// or none.
    pub fn is_parent(&self) -> Result<Master> {
        match *self {
            Fork::Child(_) => Err(ForkError::IsChild),
            Fork::Parent(_, ref master) => Ok(master.clone()),
        }
    }

    /// The function `is_child` returns the pid or child
    /// or none.
    pub fn is_child(&self) -> Result<&Slave> {
        match *self {
            Fork::Parent(_, _) => Err(ForkError::IsParent),
            Fork::Child(ref slave) => Ok(slave),
        }
    }
}

impl Drop for Fork {
    fn drop(&mut self) {
        match self {
            Fork::Parent(_, master) => Descriptor::drop(master),
            _ => {}
        }
    }
}
