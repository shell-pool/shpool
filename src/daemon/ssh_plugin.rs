use std::time;
use std::sync::{Mutex, Condvar};

use super::super::protocol;

pub const ATTACH_WINDOW: time::Duration = time::Duration::from_secs(30);

/// SshExtensionParker contains syncronization primitives to allow the
/// LocalCommand and RemoteCommand ssh extension threads to perform
/// a little handshake to hand off the name. ssh_config(5) leaves the
/// relative order in which these commands will execute unspecified,
/// so they might happen in either order or simultaneously. We must
/// be able to handle any possibility.
///
/// TODO(ethan): write unit tests for the various permutations of handshake
///              order.
/// TODO(ethan): Even with syncronization primitives in the daemon, I think
///              we can still get race conditions where a LocalCommand and
///              RemoteCommand from two different ssh invocations can
///              interleave. I think we are going to need some client side
///              locking in order to work around this, and even then I'm still
///              worried.
pub struct Parker {
    /// The empty string indicates that there is a parked thread waiting for
    pub inner: Mutex<ParkerInner>,
    pub cond: Condvar,
}

pub struct ParkerInner {
    /// The name for the session that the thread should used to attach.
    /// Set by the LocalCommandSetName thread when it wakes up the parked
    /// RemoteCommand thread.
    pub attach_header: Option<protocol::AttachHeader>,
    /// True when there is a RemoteCommand thread parked.
    pub has_parked_remote: bool,
    /// True when there is a LocalCommand thread parked.
    pub has_parked_local: bool,
}
