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

use std::time::Duration;

use parking_lot::{Condvar, Mutex};

#[derive(Debug)]
pub struct ExitNotifier {
    slot: Mutex<Option<i32>>,
    cond: Condvar,
}

impl ExitNotifier {
    pub fn new() -> Self {
        ExitNotifier { slot: Mutex::new(None), cond: Condvar::new() }
    }

    /// Notify all waiters that the process has exited.
    pub fn notify_exit(&self, status: i32) {
        let mut slot = self.slot.lock();
        *slot = Some(status);
        self.cond.notify_all();
    }

    /// Wait for the process to exit, with an optional timeout
    /// to allow the caller to wake up periodically.
    pub fn wait(&self, timeout: Option<Duration>) -> Option<i32> {
        let mut slot = self.slot.lock();

        // If a thread waits on the exit status when the child has already
        // exited, we just want to immediately return.
        if slot.is_some() {
            return *slot;
        }

        match timeout {
            Some(t) => {
                if self.cond.wait_for(&mut slot, t).timed_out() {
                    None
                } else {
                    *slot
                }
            }
            None => {
                self.cond.wait(&mut slot);
                *slot
            }
        }
    }
}
