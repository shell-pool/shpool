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
    remote_parked_at: time::Instant,
    local_parked_at: time::Instant,
    park_timeout: time::Duration,
}

impl ParkerInner {
    pub fn new(metadata: Option<Metadata>, park_timeout: time::Duration) -> Self {
        let expired_park_time = time::Instant::now().checked_sub(park_timeout).unwrap();

        ParkerInner {
            metadata,
            remote_parked_at: expired_park_time,
            local_parked_at: expired_park_time,
            park_timeout,
        }
    }

    pub fn set_has_parked_remote(&mut self, has: bool) {
        if has {
            self.remote_parked_at = time::Instant::now()
        } else {
            let expired_park_time = time::Instant::now()
                .checked_sub(self.park_timeout).unwrap();
            self.remote_parked_at = expired_park_time;
        }
    }

    pub fn has_parked_remote(&self) -> bool {
        self.remote_parked_at.elapsed() < self.park_timeout
    }

    pub fn set_has_parked_local(&mut self, has: bool) {
        if has {
            self.local_parked_at = time::Instant::now()
        } else {
            let expired_park_time = time::Instant::now()
                .checked_sub(self.park_timeout).unwrap();
            self.local_parked_at = expired_park_time;
        }
    }

    pub fn has_parked_local(&self) -> bool {
        self.local_parked_at.elapsed() < self.park_timeout
    }
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
