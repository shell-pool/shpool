// Copyright 2024 Google LLC
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

use anyhow::{anyhow, Context as _, Result};
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use notify::{
    event::ModifyKind, recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode,
    Watcher as _,
};
use std::{
    collections::{hash_map::Entry, HashMap},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tracing::{debug, error, instrument, warn};

use crate::test_hooks;

/// Watches on `path`, returnes the watched path, which is the closest existing
/// ancestor of `path`, and the immediate child that is of interest.
pub fn best_effort_watch<'a>(
    watcher: &mut RecommendedWatcher,
    path: &'a Path,
) -> Result<(&'a Path, Option<&'a Path>)> {
    let mut watched_path = Err(anyhow!("empty path"));
    // Ok or last Err
    for watch_path in path.ancestors() {
        match watcher.watch(watch_path, RecursiveMode::NonRecursive) {
            Ok(_) => {
                watched_path = Ok(watch_path);
                break;
            }
            Err(err) => watched_path = Err(err.into()),
        }
    }
    // watched path could be any ancestor of the original path
    let watched_path = watched_path.context("adding notify watch for config file")?;
    let remaining_path = path
        .strip_prefix(watched_path)
        .expect("watched_path was obtained as an ancestor of path, yet it is not a prefix");
    let immediate_child = remaining_path.iter().next();
    debug!("Actually watching {}, ic {:?}", watched_path.display(), &immediate_child);
    Ok((watched_path, immediate_child.map(Path::new)))
}

// Note that you can't add doctest for private items.
// See https://stackoverflow.com/a/76289746

/// Notify watcher to detect config file changes.
///
/// Notable features:
/// - handles non-existing config files
/// - support watching multiple files
/// - configurable debounce time for reload
///
/// For simplicity, reload doesn't distinguish which file was changed. It is
/// expected that all config files need to be reload regardless which one
/// changed.
///
/// # Examples
/// ```ignore
/// use crate::config_watcher::ConfigWatcher;
///
/// let watcher = ConfigWatcher::new(|| println!("RELOAD CONFIG")).unwrap();
/// watcher.watch("/some/path/config.toml");
/// ````
pub struct ConfigWatcher {
    /// For sending watch requests
    tx: Sender<Command>,

    /// Handle to worker thread
    #[allow(unused)]
    worker: JoinHandle<()>,

    /// For receiving debug info from worker thread, test only
    #[cfg(test)]
    debug_rx: Receiver<()>,
}

impl ConfigWatcher {
    /// Creates a new [`ConfigWatcher`] with default debounce time 100ms.
    ///
    /// Event processing happens in another thread, so the passed in `handler`
    /// is expected to properly handle synchronization and locking.
    ///
    /// # Errors
    /// Returns error if the creation of underlying `notify` watcher or worker
    /// thread failed.
    #[instrument(skip_all)]
    pub fn new(handler: impl FnMut() + Send + 'static) -> Result<Self> {
        Self::with_debounce(handler, Duration::from_millis(100))
    }

    /// Creates a new [`ConfigWatcher`] with default debounce time
    /// `reload_debounce`.
    ///
    /// Event processing happens in another thread, so the passed in `handler`
    /// is expected to properly handle synchronization and locking.
    ///
    /// # Arguments
    /// * `handler` - The handler is called when the watcher determines there is
    ///   a need to reload config files
    /// * `reload_debounce` - Reloads happen within `reload_debounce` time will
    ///   only trigger the handler once
    ///
    /// # Errors
    /// Returns error if the creation of underlying `notify` watcher or worker
    /// thread failed.
    #[instrument(skip_all)]
    pub fn with_debounce(
        handler: impl FnMut() + Send + 'static,
        reload_debounce: Duration,
    ) -> Result<Self> {
        let (notify_tx, notify_rx) = unbounded();
        let (req_tx, req_rx) = unbounded();

        #[cfg(test)]
        let (debug_tx, debug_rx) = unbounded();

        let watcher = recommended_watcher(notify_tx).context("create notify watcher")?;

        let mut inner = ConfigWatcherInner {
            reload_debounce,
            reload_deadline: None,
            handler,
            watcher,
            notify_rx,
            req_rx,
            #[cfg(test)]
            debug_tx,
            paths: Default::default(),
        };
        let worker = thread::Builder::new()
            .name("config-reload".to_string())
            .spawn(move || {
                if let Err(err) = inner.run() {
                    error!("config reload thread returned error: {:?}", err);
                }
            })
            .context("create config reload thread")?;

        Ok(Self {
            tx: req_tx,
            worker,
            #[cfg(test)]
            debug_rx,
        })
    }

    /// Adds a watch on `path`.
    ///
    /// # Errors
    /// Returns error if the underlying thread is gone, e.g. the worker thread
    /// encountered fatal error and stopped its event loop.
    #[instrument(skip_all)]
    pub fn watch(&self, path: impl AsRef<Path>) -> Result<()> {
        let (tx, rx) = bounded(1);
        self.tx
            .send(Command::AddWatch(path.as_ref().to_owned(), tx))
            .context("sending AddWatch to ConfigWatcherInner")?;
        rx.recv()?
    }

    /// Worker is idle and ready for the next event. Debug/test only.
    #[cfg(test)]
    fn worker_ready(&self) {
        self.debug_rx.recv().unwrap();
        debug!("worker ready");
    }
}

impl Drop for ConfigWatcher {
    /// Stop watching, shutting down the worker thread.
    fn drop(&mut self) {
        if let Err(err) = self.tx.send(Command::Shutdown) {
            warn!("Config watcher thread already died: {:?}", err);
        }
    }
}

/// Messages sent from `ConfigWatcher` in `ConfigWatcherInner`
enum Command {
    AddWatch(PathBuf, Sender<Result<()>>),
    Shutdown,
}

struct ConfigWatcherInner<Handler> {
    /// time to wait before actual reloading
    reload_debounce: Duration,
    /// deadline to do a reload
    reload_deadline: Option<Instant>,

    /// handle is called to signify the need to reload configs
    handler: Handler,

    /// underlying notify-rs watcher
    watcher: RecommendedWatcher,
    /// receiving notify events
    notify_rx: Receiver<Result<notify::Event, notify::Error>>,

    /// receiving watch requests from the outer `ConfigWatcher`
    req_rx: Receiver<Command>,
    /// Current watching status, it is a map from target_path to (watched_path,
    /// immediate_child_path)
    paths: HashMap<PathBuf, (PathBuf, PathBuf)>,

    /// for sending out debug info
    #[cfg(test)]
    debug_tx: Sender<()>,
}

/// Outcomes of selecting channels in the worker thread
enum Outcome {
    /// A notify event occurred
    Event(notify::Result<notify::Event>),
    /// A control command from outside
    AddWatch(PathBuf, Sender<Result<()>>),
    /// Timeout on notify event, trigger reload
    Timeout,
    /// Any channel was disconnected, or a explicit shutdown was requested
    Shutdown,
}

impl From<Command> for Outcome {
    fn from(value: Command) -> Self {
        match value {
            Command::AddWatch(path, sender) => Self::AddWatch(path, sender),
            Command::Shutdown => Self::Shutdown,
        }
    }
}

impl From<notify::Result<notify::Event>> for Outcome {
    fn from(value: notify::Result<notify::Event>) -> Self {
        Self::Event(value)
    }
}

impl<Handler> ConfigWatcherInner<Handler> {
    /// get next event to work on
    fn select(&self) -> Outcome {
        debug!("now {:?} select with ddl {:?}", Instant::now(), &self.reload_deadline);

        // only impose a deadline if there is pending reload
        let timeout = self
            .reload_deadline
            .map(crossbeam_channel::at)
            .unwrap_or_else(crossbeam_channel::never);

        #[cfg(test)]
        {
            // first try non-blocking recv, to give us a chance to to notify debug_tx about
            // we are about to go into blocking wait.
            if let Ok(res) = self.notify_rx.try_recv() {
                return Outcome::from(res);
            }
            if let Ok(res) = self.req_rx.try_recv() {
                return Outcome::from(res);
            }
            if timeout.try_recv().is_ok() {
                return Outcome::Timeout;
            }

            // nothing ready to act immediately, notify debug_tx
            self.debug_tx.send(()).unwrap();
        }

        // finally blocking wait
        select! {
            recv(self.notify_rx) -> res => res.map(Outcome::from).unwrap_or(Outcome::Shutdown),
            recv(self.req_rx) -> res => res.map(Outcome::from).unwrap_or(Outcome::Shutdown),
            recv(timeout) -> _ => Outcome::Timeout,
        }
    }

    /// Schedule a reload later.
    ///
    /// If there is already a pending deadline, it is kept as is, such that
    /// multiple reloads within `self.reload_debounce` duration only result
    /// in one actual reload. Otherwise, set the reload deadline to be
    /// `Instant::now() + self.reload_debounce`.
    fn trigger_reload(&mut self) {
        self.reload_deadline =
            self.reload_deadline.or_else(|| Some(Instant::now() + self.reload_debounce));
        debug!("defer config reloading to {:?}!", &self.reload_deadline);
    }

    /// Handle add watch command from `ConfigWatcher`.
    fn add_watch_by_command(&mut self, path: PathBuf) -> Result<()> {
        match self.paths.entry(path) {
            Entry::Occupied(e) => Err(anyhow!("{} is already being watched", e.key().display())),
            e @ Entry::Vacant(_) => {
                let reload = watch_and_add(&mut self.watcher, e)?;
                if reload {
                    self.trigger_reload();
                }
                Ok(())
            }
        }
    }

    /// Do rewatch according to the enum, return whether reload is necessary
    fn rewatch(&mut self, rewatch: ReWatch) -> bool {
        let rewatch_paths = match rewatch {
            ReWatch::Some(rewatch_paths) => rewatch_paths,
            ReWatch::All => {
                // drain paths and collect into vec first, to avoid keeping a mutable borrow on
                // self.paths
                self.paths.drain().map(|(path, (watched_path, _))| (path, watched_path)).collect()
            }
        };
        rewatch_paths.into_iter().any(|(path, watched_path)| {
            if let Err(err) = self.watcher.unwatch(&watched_path) {
                // error sometimes is expected if the watched_path was simply removed, in that
                // case notify will automatically remove the watch.
                error!("error unwatch {:?}", err);
            } else {
                debug!("unwatched {}", watched_path.display());
            }
            watch_and_add(&mut self.watcher, self.paths.entry(path))
                .map_err(|err| error!("Failed to add watch: {:?}", err))
                .unwrap_or(true)
        })
    }
}

impl<Handler> ConfigWatcherInner<Handler>
where
    Handler: FnMut(),
{
    /// Loop to reload config, only return when there is error to create any
    /// watches.
    #[instrument(skip_all)]
    fn run(&mut self) -> Result<()> {
        loop {
            match self.select() {
                Outcome::Event(res) => {
                    debug!("event: {:?}", res);
                    let (rewatch, mut reload) = match res {
                        Err(error) => {
                            error!("Error: {error:?}");
                            (ReWatch::All, false)
                        }
                        Ok(event) => handle_event(event, &self.paths),
                    };
                    debug!("rewatch = {rewatch:?}, reload = {reload}");
                    reload |= self.rewatch(rewatch);
                    if reload {
                        test_hooks::emit("daemon-config-watcher-file-change");
                        self.trigger_reload();
                    }
                }
                Outcome::AddWatch(path, sender) => {
                    debug!("addwatch: {:?}", path);
                    let _ = sender.send(self.add_watch_by_command(path));
                }
                Outcome::Timeout => {
                    debug!("timeout");
                    self.reload_deadline = None;
                    (self.handler)();
                }
                Outcome::Shutdown => {
                    debug!("stopping config watcher thread");
                    break;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ReWatch {
    /// rewatch a few (target path, watched path)
    Some(Vec<(PathBuf, PathBuf)>),
    /// rewatch all paths
    All,
}

/// Return wether need to rewatch, and whether need to reload
fn handle_event(event: Event, paths: &HashMap<PathBuf, (PathBuf, PathBuf)>) -> (ReWatch, bool) {
    if event.need_rescan() {
        debug!("need rescan");
        return (ReWatch::All, true);
    }

    // this event is about one of the watched target
    let is_original = event.paths.iter().any(|p| paths.contains_key(p));

    match event.kind {
        // create/remove in any segment in path
        EventKind::Remove(_) | EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => {
            debug!("create/remove: {:?}", event);
            // find all path entries about this event
            let rewatch = paths
                .iter()
                .filter(|(_, (watched_path, immediate_child_path))| {
                    event.paths.iter().any(|p| p == watched_path || p == immediate_child_path)
                })
                .map(|(path, (watched_path, _))| (path.to_owned(), watched_path.to_owned()))
                .collect();
            (ReWatch::Some(rewatch), is_original)
        }
        // modification in any segment in path
        EventKind::Modify(_) => {
            debug!("modify: {:?}", event);
            (ReWatch::Some(vec![]), is_original)
        }
        _ => {
            debug!("ignore {:?}", event);

            (ReWatch::Some(vec![]), false)
        }
    }
}

/// Add a watch at `path`, update paths `entry` if success, or remove `entry` if
/// failed. Note that this will overwrite any existing state.
/// Return whether reload is needed.
fn watch_and_add(
    watcher: &mut RecommendedWatcher,
    entry: Entry<PathBuf, (PathBuf, PathBuf)>,
) -> Result<bool> {
    // make a version of watch path that doesn't retain a borrow in its return value
    let best_effort_watch_owned = |watcher: &mut RecommendedWatcher, path: &Path| {
        best_effort_watch(watcher, path)
            .map(|(w, ic)| (w.to_owned(), w.join(ic.unwrap_or_else(|| Path::new("")))))
    };
    match best_effort_watch_owned(watcher, entry.key()) {
        Ok((watched_path, immediate_child_path)) => {
            let reload = &watched_path == entry.key();
            // update entry after `match watch_a_path(...)`, as that takes a borrow on entry
            // (entry.key())
            match entry {
                Entry::Occupied(mut entry) => {
                    entry.insert((watched_path, immediate_child_path));
                }
                Entry::Vacant(entry) => {
                    entry.insert((watched_path, immediate_child_path));
                }
            }
            if reload {
                debug!("Force reload since now watching on target file");
            }
            Ok(reload)
        }
        Err(err) => {
            let context_msg = format!("best_effort_watch on {}", entry.key().display());
            if let Entry::Occupied(entry) = entry {
                entry.remove();
            }
            Err(err).context(context_msg)
        }
    }
}

#[cfg(test)]
#[rustfmt::skip::attributes(test_case)]
mod test {
    use super::*;
    use ntest::timeout;
    use std::fs;
    use tempfile::TempDir;

    mod watch {
        use super::*;
        use std::fs;

        #[test]
        #[timeout(30000)]
        fn all_non_existing() {
            let mut watcher = recommended_watcher(|_| {}).unwrap();

            let (watched_path, immediate_child) =
                best_effort_watch(&mut watcher, Path::new("/non_existing/subdir")).unwrap();

            assert_eq!(watched_path, Path::new("/"));
            assert_eq!(immediate_child, Some(Path::new("non_existing")));
        }

        #[test]
        #[timeout(30000)]
        fn non_existing_parent() {
            let tmpdir = tempfile::tempdir().unwrap();
            let target_path = tmpdir.path().join(Path::new("sub1/sub2/c.txt"));

            let parent_path = target_path.parent().unwrap().parent().unwrap();

            fs::create_dir_all(parent_path).unwrap();

            let mut watcher = recommended_watcher(|_| {}).unwrap();
            let (watched_path, immediate_child) =
                best_effort_watch(&mut watcher, &target_path).unwrap();

            assert_eq!(watched_path, parent_path);
            assert_eq!(immediate_child, Some(Path::new("sub2")));
        }

        #[test]
        #[timeout(30000)]
        fn existing_file() {
            let tmpdir = tempfile::tempdir().unwrap();
            let target_path = tmpdir.path().join(Path::new("sub1/sub2/c.txt"));

            let parent_path = target_path.parent().unwrap();

            fs::create_dir_all(parent_path).unwrap();
            fs::write(&target_path, "test").unwrap();

            let mut watcher = recommended_watcher(|_| {}).unwrap();
            let (watched_path, immediate_child) =
                best_effort_watch(&mut watcher, &target_path).unwrap();

            assert_eq!(watched_path, target_path);
            assert_eq!(immediate_child, None);
        }
    }

    mod handle_event {
        use super::*;
        use assert_matches::assert_matches;
        use notify::{
            event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
            Event, EventKind,
        };
        use ntest::test_case;

        fn paths_entry(target: &str, watched: &str) -> (PathBuf, (PathBuf, PathBuf)) {
            let target = PathBuf::from(target);
            let base = PathBuf::from(watched);
            let immediate =
                base.join(target.strip_prefix(&base).unwrap().iter().next().unwrap_or_default());
            (target, (base, immediate))
        }

        // create event from spec:
        // <create|mv|modify|rm> path1 [path2]
        // `base` is prepended to all paths
        fn event_from_spec(base: &str, evt: &str) -> notify::Event {
            let base = Path::new(base);
            let (evt, path) = evt.split_once(' ').unwrap_or((evt, ""));
            match evt {
                "create" => {
                    Event::new(EventKind::Create(CreateKind::Any)).add_path(base.join(path))
                }
                "mv" => {
                    let (src, dst) = path.split_once(' ').unwrap();
                    Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                        .add_path(base.join(src))
                        .add_path(base.join(dst))
                }
                "mvselfother" => Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                    .add_path(base.to_owned())
                    .add_path(PathBuf::from("/some/other/path")),
                "modify" => {
                    Event::new(EventKind::Modify(ModifyKind::Any)).add_path(base.join(path))
                }
                "modifyself" => {
                    Event::new(EventKind::Modify(ModifyKind::Any)).add_path(base.to_owned())
                }
                "rm" => Event::new(EventKind::Remove(RemoveKind::Any)).add_path(base.join(path)),
                "rmself" => {
                    Event::new(EventKind::Remove(RemoveKind::Any)).add_path(base.to_owned())
                }
                _ => panic!("malformatted event spec"),
            }
        }

        #[test]
        #[timeout(30000)]
        fn need_rescan() {
            let event = notify::Event::default().set_flag(notify::event::Flag::Rescan);
            let paths = Default::default();
            let (rewatch, reload) = handle_event(event, &paths);
            assert_eq!(rewatch, ReWatch::All);
            assert!(reload);
        }

        const TARGET: &str = "/base/sub/config.toml";

        #[test_case(TARGET, "/base", "create sub", true, false, name = "base_create_sub")]
        #[test_case(TARGET, "/base", "create other", false, false, name = "base_create_other")]
        #[test_case(TARGET, "/base", "mv other sub", true, false, name = "base_other_to_sub")]
        #[test_case(TARGET, "/base", "mv other another", false, false, name = "base_other_to_another")]
        #[test_case(TARGET, "/base", "mv sub other", true, false, name = "base_sub_to_other")]
        #[test_case(TARGET, "/base", "rm sub", true, false, name = "base_rm_sub")]
        #[test_case(TARGET, "/base", "rm other", false, false, name = "base_rm_other")]
        #[test_case(TARGET, "/base", "modify other.toml", false, false, name = "base_modify_other")]
        #[test_case(TARGET, "/base/sub", "create config.toml", true, true, name = "sub_create_cfg")]
        #[test_case(TARGET, "/base/sub", "mv other.toml config.toml", true, true, name = "sub_other_to_cfg")]
        #[test_case(TARGET, "/base/sub", "mv other.toml another.toml", false, false, name = "sub_other_to_another")]
        #[test_case(TARGET, "/base/sub", "modify config.toml", false, true, name = "sub_modify_cfg")]
        #[test_case(TARGET, "/base/sub", "modify other.toml", false, false, name = "sub_modify_other")]
        #[test_case(TARGET, "/base/sub", "rmself", true, false, name = "sub_rm_self")]
        #[test_case(TARGET, "/base/sub/config.toml", "rmself", true, true, name = "cfg_rm_self")]
        #[test_case(TARGET, "/base/sub/config.toml", "mvselfother", true, true, name = "cfg_self_to_other")]
        #[test_case(TARGET, "/base/sub/config.toml", "modifyself", false, true, name = "cfg_modify_self")]
        #[timeout(30000)]
        fn single_path(
            target: &str,
            watched: &str,
            evt: &str,
            expected_rewatch: bool,
            expected_reload: bool,
        ) {
            let paths = HashMap::from([paths_entry(target, watched)]);
            let event = event_from_spec(watched, evt);

            let (rewatch, reload) = handle_event(event, &paths);

            let expected_rewatch = if expected_rewatch {
                ReWatch::Some(vec![(PathBuf::from(target), PathBuf::from(watched))])
            } else {
                ReWatch::Some(vec![])
            };
            assert_eq!(rewatch, expected_rewatch);
            assert_eq!(reload, expected_reload);
        }

        #[test]
        #[timeout(30000)]
        fn both_paths_are_updated() {
            let paths = HashMap::from([
                paths_entry("/base/sub/config.toml", "/base"),
                paths_entry("/base/other/another.toml", "/base"),
            ]);
            let event = event_from_spec("/base", "rm /base");

            let (rewatch, reload) = handle_event(event, &paths);

            assert_matches!(rewatch, ReWatch::Some(p) if p.len() == 2);
            assert!(!reload);
        }
    }

    // Smaller debounce time for faster testing
    const DEBOUNCE_TIME: Duration = Duration::from_millis(50);

    struct WatcherState {
        #[allow(dead_code)]
        tmpdir: TempDir,
        base_path: PathBuf,
        target_path: PathBuf,
        rx: Receiver<()>,
        watcher: ConfigWatcher,
    }

    // Setup file structure at <tmpdir>/`base`, configure watcher to watch
    // <tmpdir>/`base`/`target`
    fn setup(base: &str, target: &str) -> Result<WatcherState> {
        let tmpdir = tempfile::tempdir()?;
        let base_path = tmpdir.path().join(base);
        let target_path = base_path.join(target);
        assert!(target_path.strip_prefix(&base_path).is_ok());

        fs::create_dir_all(&base_path)?;

        let (tx, rx) = unbounded();
        let watcher = ConfigWatcher::with_debounce(move || tx.send(()).unwrap(), DEBOUNCE_TIME)?;
        watcher.watch(&target_path)?;

        Ok(WatcherState { tmpdir, base_path, target_path, rx, watcher })
    }

    // Wait for watcher to do its work and drop the watcher to close the channel
    fn drop_watcher(watcher: ConfigWatcher) {
        // sleep time larger than 1 debounce time
        thread::sleep(DEBOUNCE_TIME * 2);
        watcher.worker_ready();
    }

    #[test]
    #[timeout(30000)]
    fn debounce() {
        let state = setup("base", "sub/config.toml").unwrap();

        fs::create_dir_all(state.target_path.parent().unwrap()).unwrap();

        state.watcher.worker_ready();
        fs::write(&state.target_path, "test").unwrap();

        state.watcher.worker_ready();
        fs::write(&state.target_path, "another").unwrap();

        drop_watcher(state.watcher);

        let reloads: Vec<_> = state.rx.into_iter().collect();
        assert_eq!(reloads.len(), 1);
    }

    #[test]
    #[timeout(30000)]
    fn writes_larger_than_debounce() {
        let state = setup("base", "sub/config.toml").unwrap();

        fs::create_dir_all(state.target_path.parent().unwrap()).unwrap();
        state.watcher.worker_ready();
        fs::write(&state.target_path, "test").unwrap();

        thread::sleep(DEBOUNCE_TIME * 2);
        state.watcher.worker_ready();
        fs::write(&state.target_path, "another").unwrap();

        drop_watcher(state.watcher);

        let reloads: Vec<_> = state.rx.into_iter().collect();
        assert_eq!(reloads.len(), 2);
    }

    // /base, mv /base/other (with config.toml) /base/sub (with config.toml) =>
    // rewatch, reload
    #[test]
    #[timeout(30000)]
    fn move_multiple_levels_in_place() {
        let state = setup("base", "sub/config.toml").unwrap();

        // create /base/other/config.toml
        fs::create_dir_all(state.base_path.join("other")).unwrap();
        fs::write(state.base_path.join("other/config.toml"), "test").unwrap();

        // mv /base/other /base/sub
        fs::rename(state.base_path.join("other"), state.base_path.join("sub")).unwrap();

        drop_watcher(state.watcher);

        let reloads: Vec<_> = state.rx.into_iter().collect();
        assert_eq!(reloads.len(), 1);
    }
}
