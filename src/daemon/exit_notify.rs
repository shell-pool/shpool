use std::{
    sync::{Condvar, Mutex},
    time::Duration,
};

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
        let mut slot = self.slot.lock().unwrap();
        *slot = Some(status);
        self.cond.notify_all();
    }

    /// Wait for the process to exit, with an optional timeout
    /// to allow the caller to wake up periodically.
    pub fn wait(&self, timeout: Option<Duration>) -> Option<i32> {
        let slot = self.slot.lock().unwrap();

        // If a thread waits on the exit status when the child has already
        // exited, we just want to immediately return.
        if slot.is_some() {
            return *slot;
        }

        match timeout {
            Some(t) => {
                // returns a lock result, so we want to unwrap
                // to propagate the lock poisoning
                let (exit_status, wait_res) = self
                    .cond
                    .wait_timeout_while(slot, t, |exit_status| exit_status.is_none())
                    .unwrap();
                if wait_res.timed_out() { None } else { *exit_status }
            }
            None => *self.cond.wait_while(slot, |exit_status| exit_status.is_none()).unwrap(),
        }
    }
}
