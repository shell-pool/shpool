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

//! The common module is a grab bag of shared utility functions.

use std::{env, thread, time};

use anyhow::anyhow;

/// Controls how often `sleep_unless` re-checks its stop predicate.
#[derive(Clone, Copy, Debug)]
pub enum PollStrategy {
    /// Poll at a fixed interval.
    Uniform { interval: time::Duration },
    /// Poll with exponential backoff up to a maximum interval.
    ///
    /// Values <= 1 disable growth and behave like uniform polling.
    Backoff { initial_interval: time::Duration, factor: u32, max_interval: time::Duration },
}

/// Sleeps for up to `total_sleep`, but returns early if `stop` becomes true.
///
/// Returns `true` when `stop` triggered before timeout and `false` when the
/// full sleep window elapsed.
pub fn sleep_unless<F>(
    total_sleep: time::Duration,
    mut stop: F,
    poll_strategy: PollStrategy,
) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = time::Instant::now() + total_sleep;
    let mut next_interval = match poll_strategy {
        PollStrategy::Uniform { interval } => interval,
        PollStrategy::Backoff { initial_interval, .. } => initial_interval,
    };

    if next_interval.is_zero() {
        // Avoid a tight spin-loop if a zero interval is accidentally configured.
        next_interval = time::Duration::from_millis(1);
    }

    loop {
        if stop() {
            return true;
        }

        let now = time::Instant::now();
        if now >= deadline {
            return false;
        }

        let remaining = deadline.saturating_duration_since(now);
        thread::sleep(remaining.min(next_interval));

        if let PollStrategy::Backoff { factor, max_interval, .. } = poll_strategy {
            if factor > 1 {
                let grown = next_interval.checked_mul(factor).unwrap_or(max_interval);
                next_interval = grown.min(max_interval);
            }
        }
    }
}

pub fn resolve_sessions(sessions: &mut Vec<String>, action: &str) -> anyhow::Result<()> {
    if sessions.is_empty() {
        if let Ok(current_session) = env::var("SHPOOL_SESSION_NAME") {
            sessions.push(current_session);
        }
    }

    if sessions.is_empty() {
        eprintln!("no session to {action}");
        return Err(anyhow!("no session to {action}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::time::Duration;

    use super::{sleep_unless, PollStrategy};

    #[test]
    fn sleep_unless_returns_immediately_when_stop_is_true() {
        let stopped = sleep_unless(
            Duration::from_millis(10),
            || true,
            PollStrategy::Uniform { interval: Duration::from_millis(1) },
        );

        assert!(stopped);
    }

    #[test]
    fn sleep_unless_times_out_when_stop_is_false() {
        let stopped = sleep_unless(
            Duration::from_millis(3),
            || false,
            PollStrategy::Uniform { interval: Duration::from_millis(1) },
        );

        assert!(!stopped);
    }

    #[test]
    fn sleep_unless_rechecks_stop_with_backoff() {
        let checks = Cell::new(0usize);
        let stopped = sleep_unless(
            Duration::from_millis(20),
            || {
                let n = checks.get() + 1;
                checks.set(n);
                n >= 3
            },
            PollStrategy::Backoff {
                initial_interval: Duration::from_millis(1),
                factor: 2,
                max_interval: Duration::from_millis(4),
            },
        );

        assert!(stopped);
        assert!(checks.get() >= 3);
    }
}
