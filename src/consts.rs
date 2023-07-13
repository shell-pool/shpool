use std::time;

pub const SOCK_STREAM_TIMEOUT: time::Duration = time::Duration::from_millis(200);
pub const JOIN_POLL_DURATION: time::Duration = time::Duration::from_millis(100);

pub const BUF_SIZE: usize = 1024 * 16;

pub const HEARTBEAT_DURATION: time::Duration = time::Duration::from_millis(500);
