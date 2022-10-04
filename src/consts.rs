use std::time;

pub const SOCK_STREAM_TIMEOUT: time::Duration = time::Duration::from_millis(200);
pub const JOIN_POLL_DURATION: time::Duration = time::Duration::from_millis(100);

pub const BUF_SIZE: usize = 1024 * 16; // 16k buffers

const PIPE_POLL_MILLIS: u64 = 100;
pub const PIPE_POLL_DURATION: time::Duration = time::Duration::from_millis(PIPE_POLL_MILLIS);
pub const PIPE_POLL_DURATION_TIMEVAL: nix::sys::time::TimeVal =
    nix::sys::time::TimeVal::new(0, 1000 * (PIPE_POLL_MILLIS as nix::sys::time::suseconds_t));

pub const HEARTBEAT_DURATION: time::Duration = time::Duration::from_millis(200);
