use ::descriptor::DescriptorError;
use std::error::Error;
use std::fmt;

use super::pty::{MasterError, SlaveError};

/// The alias `Result` learns `ForkError` possibility.
pub type Result<T> = ::std::result::Result<T, ForkError>;

/// The enum `ForkError` defines the possible errors from constructor Fork.
#[derive(Clone, Copy, Debug)]
pub enum ForkError {
    /// Can't creates the child.
    Failure,
    /// Can't set the id group.
    SetsidFail,
    /// Can't suspending the calling process.
    WaitpidFail,
    /// Is child and not parent.
    IsChild,
    /// Is parent and not child.
    IsParent,
    /// The Master occured a error.
    BadMaster(MasterError),
    /// The Slave occured a error.
    BadSlave(SlaveError),
    /// The Master's Descriptor occured a error.
    BadDescriptorMaster(DescriptorError),
    /// The Slave's Descriptor occured a error.
    BadDescriptorSlave(DescriptorError),
    /// Cannot create a fork from a null ptsname
    BadPtsname,
}

impl fmt::Display for ForkError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for ForkError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            ForkError::Failure => {
                "On failure, -1 is returned in the parent,no child process is created, and errno \
                 isset appropriately."
            }
            ForkError::SetsidFail => {
                "fails if the calling process is alreadya process group leader."
            }
            ForkError::WaitpidFail => "Can't suspending the calling process.",
            ForkError::IsChild => "is child and not parent",
            ForkError::IsParent => "is parent and not child",
            ForkError::BadMaster(_) => "the master as occured an error",
            ForkError::BadSlave(_) => "the slave as occured an error",
            ForkError::BadDescriptorMaster(_) => "the master's descriptor as occured an error",
            ForkError::BadDescriptorSlave(_) => "the slave's descriptor as occured an error",
            ForkError::BadPtsname => "null ptsname",

        }
    }

    /// The function `cause` returns the lower-level cause of this error, if any.
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            ForkError::BadMaster(ref err) => Some(err),
            ForkError::BadSlave(ref err) => Some(err),
            ForkError::BadDescriptorMaster(ref err) => Some(err),
            ForkError::BadDescriptorSlave(ref err) => Some(err),
            _ => None,
        }
    }
}
