use ::descriptor::DescriptorError;
use std::error::Error;
use std::fmt;

/// The alias `Result` learns `MasterError` possibility.

pub type Result<T> = ::std::result::Result<T, MasterError>;

/// The enum `MasterError` defines the possible errors from constructor Master.
#[derive(Clone, Copy, Debug)]
pub enum MasterError {
    BadDescriptor(DescriptorError),
    GrantptError,
    UnlockptError,
    PtsnameError,
}

impl fmt::Display for MasterError {
    /// The function `fmt` formats the value using the given formatter.

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for MasterError {
    /// The function `description` returns a short description of the error.

    fn description(&self) -> &str {
        match *self {
            MasterError::BadDescriptor(_) => "the descriptor as occured an error",
            MasterError::GrantptError => "the `grantpt` has a error, errnois set appropriately.",
            MasterError::UnlockptError => "the `grantpt` has a error, errnois set appropriately.",
            MasterError::PtsnameError => "the `ptsname` has a error",

        }
    }

    /// The function `cause` returns the lower-level cause of this error, if any.
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            MasterError::BadDescriptor(ref err) => Some(err),
            _ => None,
        }
    }
}
