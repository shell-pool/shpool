use std::{io, io::BufRead, os::unix::net::UnixStream, path::Path, time};

use anyhow::anyhow;

/// Event represents a stream of events you can wait for.
///
/// To actually wait for a particular event, you should create
/// an EventWaiter with the `waiter` or `await_event` routines.
pub struct Events {
    lines: io::Lines<io::BufReader<UnixStream>>,
}

impl Events {
    pub fn new<P: AsRef<Path>>(sock: P) -> anyhow::Result<Self> {
        let mut sleep_dur = time::Duration::from_millis(5);
        for _ in 0..12 {
            if let Ok(s) = UnixStream::connect(&sock) {
                return Ok(Events { lines: io::BufReader::new(s).lines() });
            } else {
                std::thread::sleep(sleep_dur);
                sleep_dur *= 2;
            }
        }

        Err(anyhow!("timed out waiting for connection to event sock"))
    }

    /// waiter creates an event waiter that can later be used to
    /// block until the event occurs. You should generally call waiter
    /// before you take the action that will trigger the event in order
    /// to avoid race conditions.
    ///
    /// `events` should be a list of events to listen for, in order.
    /// You can wait for the events by calling methods on the EventWaiter,
    /// and you should make sure to use `wait_final_event` to get the
    /// Events struct back at the last event.
    pub fn waiter<S, SI>(mut self, events: SI) -> EventWaiter
    where
        S: Into<String>,
        SI: IntoIterator<Item = S>,
    {
        let events: Vec<String> = events.into_iter().map(|s| s.into()).collect();
        assert!(!events.is_empty());

        let (tx, rx) = crossbeam_channel::bounded(events.len());
        let waiter = EventWaiter { matched: rx };
        std::thread::spawn(move || {
            let mut return_lines = false;
            let mut offset = 0;

            'LINELOOP: for line in &mut self.lines {
                match line {
                    Ok(l) => {
                        if events[offset] == l {
                            if offset == events.len() - 1 {
                                // this is the last event
                                return_lines = true;
                                break 'LINELOOP;
                            } else {
                                tx.send(WaiterEvent::Event(l)).unwrap();
                            }
                            offset += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("error scanning for event '{}': {:?}", events[offset], e);
                    }
                }
            }

            if return_lines {
                tx.send(WaiterEvent::Done((events[offset].clone(), self.lines))).unwrap();
            }
        });

        waiter
    }

    /// await_events waits for a given event on the stream.
    /// Prefer `waiter` since it is less prone to race conditions.
    /// `await_event` might be approriate for startup events where
    /// it is not possible to use `waiter`.
    pub fn await_event(&mut self, event: &str) -> anyhow::Result<()> {
        for line in &mut self.lines {
            let line = line?;
            if line == event {
                return Ok(());
            }
        }

        Ok(())
    }
}

/// EventWaiter represents waiting for a particular event.
/// It should be converted back into an Events struct with
/// the wait() routine.
pub struct EventWaiter {
    matched: crossbeam_channel::Receiver<WaiterEvent>,
}

enum WaiterEvent {
    Event(String),
    Done((String, io::Lines<io::BufReader<UnixStream>>)),
}

impl EventWaiter {
    pub fn wait_event(&mut self, event: &str) -> anyhow::Result<()> {
        eprintln!("waiting for event '{event}'");
        match self.matched.recv()? {
            WaiterEvent::Event(e) => {
                if e == event {
                    Ok(())
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                }
            }
            WaiterEvent::Done((e, _)) => {
                if e == event {
                    Ok(())
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                }
            }
        }
    }

    pub fn wait_final_event(self, event: &str) -> anyhow::Result<Events> {
        eprintln!("waiting for final event '{event}'");
        match self.matched.recv()? {
            WaiterEvent::Event(e) => {
                Err(anyhow!("Got non-fianl '{}' event, want final '{}'", e, event))
            }
            WaiterEvent::Done((e, lines)) => {
                if e == event {
                    Ok(Events { lines })
                } else {
                    Err(anyhow!("Got '{}' event, want '{}'", e, event))
                }
            }
        }
    }
}
