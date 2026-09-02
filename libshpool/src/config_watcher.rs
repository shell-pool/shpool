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
use notify::{event::ModifyKind, RecursiveMode, Watcher as _};
use std::{
    collections::{hash_map::Entry, HashMap},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::test_hooks;

/// Canonicalize a path, resolving symlinks in the existing portion.
///
/// This is needed because file system watchers (inotify, FSEvents, etc.) report
/// canonical paths, so we need to store canonical paths for comparison.
/// Unlike `std::fs::canonicalize`, this handles paths where the final
/// components don't exist yet by canonicalizing the longest existing prefix.
fn canonicalize_path(path: &Path) -> PathBuf {
    // Try to canonicalize the whole path first
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    // Find the longest existing ancestor and canonicalize that
    for ancestor in path.ancestors().skip(1) {
        if let Ok(canonical_ancestor) = ancestor.canonicalize() {
            // Append the remaining (non-existent) components
            if let Ok(remaining) = path.strip_prefix(ancestor) {
                return canonical_ancestor.join(remaining);
            }
        }
    }

    // Fallback to original path if nothing could be canonicalized
    path.to_path_buf()
}

// Note that you can't add doctest for private items.
// See https://stackoverflow.com/a/76289746

/// Configuration options for [`ConfigWatcher`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Time to wait after a filesystem event before triggering a reload.
    pub debounce: Duration,
    /// Interval at which missing config file paths are checked for existence.
    pub poll_interval: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(100),
            // Parent paths can change much more frequently than target watch
            // files, so if the target file doesn't exist, rather than watching
            // parent dirs directly, we instead wake up at least every poll dur
            // and check if the target files do exist. If they do, we install proper
            // watchers.
            poll_interval: Duration::from_secs(5),
        }
    }
}

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
    /// Creates a new [`ConfigWatcher`] with default options.
    ///
    /// Event processing happens in another thread, so the passed in `handler`
    /// is expected to properly handle synchronization and locking.
    ///
    /// # Errors
    /// Returns error if the creation of underlying `notify` watcher or worker
    /// thread failed.
    #[instrument(skip_all)]
    pub fn new(handler: impl FnMut() + Send + 'static) -> Result<Self> {
        Self::with_options(handler, Options::default())
    }

    /// Creates a new [`ConfigWatcher`] with the given [`Options`].
    ///
    /// Event processing happens in another thread, so the passed in `handler`
    /// is expected to properly handle synchronization and locking.
    ///
    /// # Errors
    /// Returns error if the creation of underlying `notify` watcher or worker
    /// thread failed.
    #[instrument(skip_all)]
    pub fn with_options(handler: impl FnMut() + Send + 'static, options: Options) -> Result<Self> {
        let (notify_tx, notify_rx) = unbounded();
        let (req_tx, req_rx) = unbounded();

        #[cfg(test)]
        let (debug_tx, debug_rx) = unbounded();

        let watcher = notify::recommended_watcher(notify_tx).context("create notify watcher")?;

        let mut inner = ConfigWatcherInner {
            reload_debounce: options.debounce,
            reload_deadline: None,
            poll_interval: options.poll_interval,
            handler,
            watcher,
            notify_rx,
            req_rx,
            #[cfg(test)]
            debug_tx,
            last_paths_presence_poll: Instant::now(),
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
    /// interval to poll for missing paths
    poll_interval: Duration,

    /// handle is called to signify the need to reload configs
    handler: Handler,

    /// A table mapping target paths to a flag indicating if the
    /// underlying watcher is actually watching the path or not.
    /// A false entry indicates that we should check to see if
    /// this path exists every poll_interval and install
    /// a watcher if the user has created the file.
    paths: HashMap<PathBuf, bool>,

    /// underlying notify-rs watcher
    watcher: notify::RecommendedWatcher,

    /// receiving notify events
    notify_rx: Receiver<Result<notify::Event, notify::Error>>,

    /// receiving watch requests from the outer `ConfigWatcher`
    req_rx: Receiver<Command>,

    // The last time we checked in on all the target paths that
    // don't have a watcher installed.
    last_paths_presence_poll: Instant,

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
    /// Timeout indicating that the debounce period is up and the
    /// reload handler needs to be called.
    DebounceTimeout,
    /// Timeout indicating that we need to recheck paths that we were
    /// unable to install a watcher for.
    RescanTimeout,
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
        trace!("now {:?} select with ddl {:?}", Instant::now(), &self.reload_deadline);

        // only impose a deadline if there is pending reload
        let debounce_timeout = self
            .reload_deadline
            .map(crossbeam_channel::at)
            .unwrap_or_else(crossbeam_channel::never);

        let rescan_timeout =
            crossbeam_channel::at(self.last_paths_presence_poll + self.poll_interval);

        #[cfg(test)]
        {
            // first try non-blocking recv, to give us a chance to to notify
            // debug_tx about we are about to go into blocking wait.
            if let Ok(res) = self.notify_rx.try_recv() {
                return Outcome::from(res);
            }
            if let Ok(res) = self.req_rx.try_recv() {
                return Outcome::from(res);
            }
            if debounce_timeout.try_recv().is_ok() {
                return Outcome::DebounceTimeout;
            }
            if rescan_timeout.try_recv().is_ok() {
                return Outcome::RescanTimeout;
            }

            // Only signal idle if there's no pending reload deadline.
            // If there's a pending deadline, we have work to do (wait for
            // timeout).
            if self.reload_deadline.is_none() {
                self.debug_tx.send(()).unwrap();
            }
        }

        // finally blocking wait
        select! {
            recv(self.notify_rx) -> res => res.map(Outcome::from).unwrap_or(Outcome::Shutdown),
            recv(self.req_rx) -> res => res.map(Outcome::from).unwrap_or(Outcome::Shutdown),
            recv(debounce_timeout) -> _ => Outcome::DebounceTimeout,
            recv(rescan_timeout) -> _ => Outcome::RescanTimeout,
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

    fn remove_watch(&mut self, path: PathBuf) -> Result<()> {
        if let Entry::Occupied(mut e) = self.paths.entry(path.clone()) {
            e.insert(false);
        }

        // error sometimes is expected if the watched_path was
        // simply removed, in that case notify will automatically
        // remove the watch.
        self.watcher.unwatch(&path).context("removing file watcher")
    }

    /// Handle add watch command from `ConfigWatcher`.
    fn add_watch(&mut self, path: PathBuf) -> Result<()> {
        let canonical_path = canonicalize_path(&path);
        match self.paths.entry(canonical_path.clone()) {
            Entry::Occupied(e) => Err(anyhow!("{} is already being watched", e.key().display())),
            entry @ Entry::Vacant(_) => {
                if canonical_path.exists() {
                    if let Err(err) =
                        self.watcher.watch(&canonical_path, RecursiveMode::NonRecursive)
                    {
                        warn!("error watching {:?}: {:?}", canonical_path, err);
                        entry.insert_entry(false);
                    } else {
                        entry.insert_entry(true);
                    }
                } else {
                    entry.insert_entry(false);
                }

                Ok(())
            }
        }
    }

    fn recheck_missing_paths(&mut self) -> Result<()> {
        self.last_paths_presence_poll = Instant::now();
        let mut reload = false;
        for (path, has_watcher) in self.paths.iter_mut() {
            if !*has_watcher && path.exists() {
                if let Err(err) = self.watcher.watch(path, RecursiveMode::NonRecursive) {
                    warn!("error watching {:?}: {:?}", path, err);
                } else {
                    *has_watcher = true;
                    reload = true;
                }
            }
        }
        if reload {
            self.trigger_reload();
        }

        Ok(())
    }

    /// Do rewatch according to the enum, return whether reload is necessary
    fn rewatch(&mut self, rewatch: ReWatch) -> bool {
        let rewatch_paths = match rewatch {
            ReWatch::Some(rewatch_paths) => rewatch_paths,
            ReWatch::All => self.paths.keys().cloned().collect::<Vec<_>>(),
        };
        rewatch_paths.into_iter().any(|path| {
            if let Err(err) = self.remove_watch(path.clone()) {
                error!("error unwatch {:?}", err);
            } else {
                debug!("unwatched {}", path.display());
            }

            if path.exists() {
                if let Err(err) = self.watcher.watch(&path, RecursiveMode::NonRecursive) {
                    warn!("error watching {:?}: {:?}", path, err);
                    false
                } else {
                    if let Some(has_watcher) = self.paths.get_mut(&path) {
                        *has_watcher = true;
                    }
                    true
                }
            } else {
                false
            }
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
                    trace!("event: {:?}", res);
                    let (rewatch, mut reload) = match res {
                        Err(error) => {
                            error!("Error: {error:?}");
                            (ReWatch::All, false)
                        }
                        Ok(event) => handle_event(event),
                    };
                    trace!("rewatch = {rewatch:?}, reload = {reload}");
                    reload |= self.rewatch(rewatch);
                    if reload {
                        test_hooks::emit("daemon-config-watcher-file-change");
                        self.trigger_reload();
                    }
                }
                Outcome::AddWatch(path, sender) => {
                    trace!("addwatch: {:?}", path);
                    let _ = sender.send(self.add_watch(path));
                }
                Outcome::DebounceTimeout => {
                    trace!("debounce timeout");
                    self.reload_deadline = None;
                    (self.handler)();
                }
                Outcome::RescanTimeout => {
                    if let Err(e) = self.recheck_missing_paths() {
                        warn!("while rechecking missing paths: {:?}", e);
                    }
                }
                Outcome::Shutdown => {
                    info!("stopping config watcher thread");
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
    Some(Vec<PathBuf>),
    /// rewatch all paths
    All,
}

/// Return wether need to rewatch, and whether need to reload
fn handle_event(event: notify::Event) -> (ReWatch, bool) {
    if event.need_rescan() {
        debug!("need rescan");
        return (ReWatch::All, true);
    }

    match event.kind {
        // create/remove in any segment in path
        notify::EventKind::Remove(_)
        | notify::EventKind::Create(_)
        | notify::EventKind::Modify(ModifyKind::Name(_)) => {
            debug!("create/remove: {:?}", event);
            (ReWatch::Some(event.paths), true)
        }
        // modification in any segment in path
        notify::EventKind::Modify(_) => {
            debug!("modify: {:?}", event);
            (ReWatch::Some(vec![]), true)
        }
        _ => {
            debug!("ignore {:?}", event);

            (ReWatch::Some(vec![]), false)
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

    mod handle_event {
        use super::*;
        use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode};

        #[test]
        #[timeout(30000)]
        fn need_rescan() {
            let event = notify::Event::default().set_flag(notify::event::Flag::Rescan);
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::All);
            assert!(reload);
        }

        #[test]
        #[timeout(30000)]
        fn create_event() {
            let path = PathBuf::from("/some/config.toml");
            let event = notify::Event::new(notify::EventKind::Create(CreateKind::Any))
                .add_path(path.clone());
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::Some(vec![path]));
            assert!(reload);
        }

        #[test]
        #[timeout(30000)]
        fn modify_event() {
            let path = PathBuf::from("/some/config.toml");
            let event =
                notify::Event::new(notify::EventKind::Modify(ModifyKind::Any)).add_path(path);
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::Some(vec![]));
            assert!(reload);
        }

        #[test]
        #[timeout(30000)]
        fn remove_event() {
            let path = PathBuf::from("/some/config.toml");
            let event = notify::Event::new(notify::EventKind::Remove(RemoveKind::Any))
                .add_path(path.clone());
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::Some(vec![path]));
            assert!(reload);
        }

        #[test]
        #[timeout(30000)]
        fn rename_event() {
            let path1 = PathBuf::from("/some/config.toml");
            let path2 = PathBuf::from("/some/config.toml.bak");
            let event =
                notify::Event::new(notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                    .add_path(path1.clone())
                    .add_path(path2.clone());
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::Some(vec![path1, path2]));
            assert!(reload);
        }

        #[test]
        #[timeout(30000)]
        fn ignore_other_event() {
            let path = PathBuf::from("/some/config.toml");
            let event =
                notify::Event::new(notify::EventKind::Access(AccessKind::Any)).add_path(path);
            let (rewatch, reload) = handle_event(event);
            assert_eq!(rewatch, ReWatch::Some(vec![]));
            assert!(!reload);
        }
    }

    // Smaller debounce and poll times for faster testing
    const DEBOUNCE_TIME: Duration = Duration::from_millis(50);
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    struct WatcherState {
        #[allow(dead_code)]
        tmpdir: TempDir,
        #[allow(dead_code)]
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

        fs::create_dir_all(target_path.parent().unwrap())?;
        fs::write(&target_path, "initial")?;

        let (tx, rx) = unbounded();
        let watcher = ConfigWatcher::with_options(
            move || tx.send(()).unwrap(),
            Options { debounce: DEBOUNCE_TIME, poll_interval: POLL_INTERVAL },
        )?;
        watcher.watch(&target_path)?;

        Ok(WatcherState { tmpdir, base_path, target_path, rx, watcher })
    }

    // Wait for watcher to do its work and drop the watcher to close the channel
    fn drop_watcher(watcher: ConfigWatcher) {
        thread::sleep(DEBOUNCE_TIME * 2);
        watcher.worker_ready();
    }

    #[test]
    #[timeout(30000)]
    #[cfg_attr(target_os = "macos", ignore)]
    fn debounce() {
        let state = setup("base", "sub/config.toml").unwrap();

        state.watcher.worker_ready();
        // Write twice in quick succession - both should be within debounce
        // window
        fs::write(&state.target_path, "test").unwrap();
        fs::write(&state.target_path, "another").unwrap();

        drop_watcher(state.watcher);

        let reloads: Vec<_> = state.rx.into_iter().collect();
        assert_eq!(reloads.len(), 1);
    }

    #[test]
    #[timeout(30000)]
    fn writes_larger_than_debounce() {
        let state = setup("base", "sub/config.toml").unwrap();

        state.watcher.worker_ready();
        fs::write(&state.target_path, "test").unwrap();

        thread::sleep(DEBOUNCE_TIME * 2);
        state.watcher.worker_ready();
        fs::write(&state.target_path, "another").unwrap();

        drop_watcher(state.watcher);

        let reloads: Vec<_> = state.rx.into_iter().collect();
        assert_eq!(reloads.len(), 2);
    }

    #[test]
    #[timeout(30000)]
    fn missing_file_discovered_by_polling() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target_path = tmpdir.path().join("sub/config.toml");
        fs::create_dir_all(target_path.parent().unwrap()).unwrap();

        let (tx, rx) = unbounded();
        let watcher = ConfigWatcher::with_options(
            move || tx.send(()).unwrap(),
            Options { debounce: DEBOUNCE_TIME, poll_interval: POLL_INTERVAL },
        )
        .unwrap();
        watcher.watch(&target_path).unwrap();

        watcher.worker_ready();
        // Create the file after the watcher is already running
        fs::write(&target_path, "created").unwrap();

        thread::sleep(POLL_INTERVAL * 4 + DEBOUNCE_TIME * 2);

        drop_watcher(watcher);

        let reloads: Vec<_> = rx.into_iter().collect();
        assert_eq!(
            reloads.len(),
            1,
            "expected 1 reload after file creation, got {}",
            reloads.len()
        );
    }

    #[test]
    #[timeout(30000)]
    fn already_watched_error() {
        let state = setup("base", "sub/config.toml").unwrap();
        let err = state.watcher.watch(&state.target_path);
        assert!(err.is_err());
    }

    /// Regression test: ConfigWatcher should resolve symlinks in watched paths.
    #[test]
    #[timeout(30000)]
    #[cfg_attr(target_os = "macos", ignore)]
    fn symlink_path_is_canonicalized() {
        use std::os::unix::fs::symlink;

        let tmpdir = tempfile::tempdir().unwrap();

        // setup: real dir + symlink to it
        let real_dir = tmpdir.path().join("real");
        fs::create_dir_all(&real_dir).unwrap();
        let link_dir = tmpdir.path().join("link");
        symlink(&real_dir, &link_dir).unwrap();

        let real_target = real_dir.join("config.toml");
        fs::write(&real_target, "initial").unwrap();

        // watch through the symlink
        let symlinked_target = link_dir.join("config.toml");
        let (tx, rx) = unbounded();
        let watcher = ConfigWatcher::with_options(
            move || tx.send(()).unwrap(),
            Options { debounce: DEBOUNCE_TIME, ..Default::default() },
        )
        .unwrap();
        watcher.watch(&symlinked_target).unwrap();

        watcher.worker_ready();
        fs::write(&symlinked_target, "test content").unwrap();

        drop_watcher(watcher);

        let reloads: Vec<_> = rx.into_iter().collect();
        assert_eq!(reloads.len(), 1, "expected 1 reload, got {}", reloads.len());
    }
}
