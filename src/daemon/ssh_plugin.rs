use std::time;
use std::sync::{Mutex, Condvar};

/// SshExtensionParker contains syncronization primitives to allow the
/// LocalCommand and RemoteCommand ssh extension threads to perform
/// a little handshake to hand off the name. ssh_config(5) leaves the
/// relative order in which these commands will execute unspecified,
/// so they might happen in either order or simultaneously. We must
/// be able to handle any possibility.
///
/// TODO(ethan): Even with syncronization primitives in the daemon,
///              I think we can still get race conditions where a
///              LocalCommand and RemoteCommand from two different
///              ssh invocations can interleave. I think we are
///              going to need some client side locking in order
///              to work around this, and even then I'm still
///              worried.
pub struct Parker {
    /// The empty string indicates that there is a parked thread waiting for
    pub inner: Mutex<ParkerInner>,
    pub cond: Condvar,
}

#[derive(Debug)]
pub struct ParkerInner {
    /// The name for the session that the thread should used
    /// to attach. Set by the LocalCommandSetName thread
    /// when it wakes up the parked RemoteCommand thread.
    pub metadata: Option<Metadata>,
    /// True when there is a RemoteCommand thread parked.
    pub has_parked_remote: bool,
    /// True when there is a LocalCommand thread parked.
    pub has_parked_local: bool,
}

#[derive(Debug)]
pub struct Metadata {
    pub name: String,
    pub term: String,
    pub set_at: time::Instant,
}

impl std::default::Default for Metadata {
    fn default() -> Self {
        Metadata {
            name: String::from(""),
            term: String::from(""),
            set_at: time::Instant::now().checked_sub(
                time::Duration::from_secs(60*60*24))
                .unwrap_or(time::Instant::now()),
        }
    }
}
