/*! The ttl reaper is responsible to reaping sessions which
  have a ttl set. It uses a channel for a mailbox to listen
  for newly woken threads, adds a generation id to session
  names to avoid clobbering fresh session with the same
  session name as a previous session, and uses a min heap
  to schedule wakeups in order to reap threads on time.
*/

use std::{
    cmp,
    collections::{BinaryHeap, HashMap},
    sync::{Arc, Mutex},
    time::Instant,
};

use tracing::{info, span, warn, Level};

use super::shell;

/// Run the reaper thread loop. Should be invoked in a dedicated
/// thread.
pub fn run(
    new_sess: crossbeam_channel::Receiver<(String, Instant)>,
    shells: Arc<Mutex<HashMap<String, Box<shell::Session>>>>,
) -> anyhow::Result<()> {
    let _s = span!(Level::INFO, "ttl_reaper").entered();

    let mut heap = BinaryHeap::new();
    let mut gen_ids = HashMap::new();

    loop {
        // empty heap loop, just waiting for new sessions to watch
        while heap.len() == 0 {
            match new_sess.recv() {
                Ok((session_name, reap_at)) => {
                    let gen_id = gen_ids.entry(session_name.clone()).or_insert(0);
                    *gen_id += 1;
                    info!(
                        "scheduling first sess {}:{} to be reaped at {:?}",
                        &session_name, *gen_id, reap_at
                    );
                    heap.push(Reapable { session_name, gen_id: *gen_id, reap_at });
                }
                Err(crossbeam_channel::RecvError) => {
                    info!("bailing due to RecvError in empty heap loop");
                    return Ok(());
                }
            }
        }

        while heap.len() > 0 {
            let wake_at = if let Some(reapable) = heap.peek() {
                reapable.reap_at.clone()
            } else {
                warn!("no reapable even with heap len {}, should be impossible", heap.len());
                continue;
            };

            crossbeam_channel::select! {
                recv(new_sess) -> new_sess_msg => {
                    match new_sess_msg {
                        Ok((session_name, reap_at)) => {
                            let gen_id = gen_ids.entry(session_name.clone()).or_insert(0);
                            *gen_id += 1;
                            info!("scheduling {}:{} to be reaped at {:?}",
                                  &session_name, *gen_id, reap_at);
                            heap.push(Reapable {
                                session_name,
                                gen_id: *gen_id,
                                reap_at,
                            });
                        }
                        Err(crossbeam_channel::RecvError) => {
                            info!("bailing due to RecvError");
                            return Ok(())
                        },
                    }
                }
                recv(crossbeam_channel::at(wake_at)) -> _ => {
                    let reapable = heap.pop()
                        .expect("there to be an entry in a non-empty heap");
                    info!("waking up to reap {:?}", reapable);
                    let current_gen = gen_ids.get(&reapable.session_name)
                        .map(|g| *g).unwrap_or(0);
                    if current_gen != reapable.gen_id {
                        info!("ignoring {}:{} because current gen is {:?}",
                              &reapable.session_name, reapable.gen_id, current_gen);
                        continue;
                    }

                    let mut shells = shells.lock().unwrap();
                    if let Some(sess) = shells.get(&reapable.session_name) {
                        if let Err(e) = sess.kill() {
                            warn!("error trying to kill '{}': {:?}",
                                  reapable.session_name, e);
                        }
                    } else {
                        warn!("tried to kill '{}' but it wasn't in the shells tab",
                              reapable.session_name);
                        continue;
                    }
                    shells.remove(&reapable.session_name);
                }
            }
        }
    }
}

/// A record in the min heap that we use to track the
/// sessions that need to be cleaned up.
#[derive(Debug)]
struct Reapable {
    session_name: String,
    gen_id: usize,
    reap_at: Instant,
}

impl cmp::PartialEq for Reapable {
    fn eq(&self, rhs: &Reapable) -> bool {
        return self.reap_at == rhs.reap_at;
    }
}
impl cmp::Eq for Reapable {}

impl cmp::PartialOrd for Reapable {
    fn partial_cmp(&self, other: &Reapable) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for Reapable {
    fn cmp(&self, other: &Reapable) -> cmp::Ordering {
        // flip the ordering to make a min heap
        other.reap_at.cmp(&self.reap_at)
    }
}
