// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::time;

pub const SOCK_STREAM_TIMEOUT: time::Duration = time::Duration::from_millis(200);
pub const JOIN_POLL_DURATION: time::Duration = time::Duration::from_millis(100);

pub const BUF_SIZE: usize = 1024 * 16;

pub const HEARTBEAT_DURATION: time::Duration = time::Duration::from_millis(500);

pub const STDIN_FD: i32 = 0;
pub const STDERR_FD: i32 = 2;

// Used to determine when the shell has started up so we can attempt to sniff
// what type of shell it is based on /proc/<pid>/exe.
pub const STARTUP_SENTINEL: &str = "SHPOOL_STARTUP_SENTINEL";

// Used to flag when prompt setup is complete and we can stop
// dropping the output.
pub const PROMPT_SENTINEL: &str = "SHPOOL_PROMPT_SETUP_SENTINEL";

// A magic env var which indicates that a `shpool daemon` invocation should
// actually just print the given sentinal then exit. We do this because
// using `echo` will cause the sentinal value to appear multiple times
// in the output stream. For the same reason, we don't set the value
// to an actual sentianl, but instead either "startup" or "prompt".
pub const SENTINEL_FLAG_VAR: &str = "SHPOOL__INTERNAL__PRINT_SENTINEL";

// If set to "true", the daemon will autodaemonize after launch.
pub const AUTODAEMONIZE_VAR: &str = "SHPOOL__INTERNAL__AUTODAEMONIZE";
