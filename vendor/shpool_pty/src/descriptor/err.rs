use std::error::Error;
use std::fmt;

/// The enum `DescriptorError` defines the possible errors
/// from constructor Descriptor.
#[derive(Clone, Copy, Debug)]
pub enum DescriptorError {
    /// Can't open.
    OpenFail,
    /// Can't closed.
    CloseFail,
}

impl fmt::Display for DescriptorError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for DescriptorError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            DescriptorError::OpenFail => "can't open the fd",
            DescriptorError::CloseFail => "can't close the fd",
        }
    }

    /// The function `cause` returns the lower-level cause of this error, if any.

    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}
