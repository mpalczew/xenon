//! Git state for every open workspace, measured off the main thread.
//!
//! One worker thread owns the filesystem watcher and runs `xenon_git::measure`.
//! It sleeps in a blocking receive, so an idle app (and a dirty repo nobody is
//! touching) costs no wakeups. File events are debounced, and one workspace is
//! measured at most once per `MIN_GAP`, so a build or a save burst makes a few
//! git calls, never one per event. The main thread only receives finished
//! statuses.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gtk::glib;
use notify::{EventKind, RecursiveMode, Watcher, event::ModifyKind};
use xenon_core::WorkspaceId;
use xenon_git::status::{DirtState, GitStatus};
use xenon_git::watch::DirSet;

use crate::app::App;

/// Quiet time after the last event before git runs.
const DEBOUNCE: Duration = Duration::from_millis(400);
/// Least time between two measurements of one workspace.
const MIN_GAP: Duration = Duration::from_millis(1500);
/// When no file watcher could be made (the machine is out of inotify
/// instances, which a busy desktop can be), look again this often.
const POLL: Duration = Duration::from_secs(10);

type Roots = Vec<(WorkspaceId, PathBuf)>;
/// `None` status: the folder is not a git work tree (or was closed).
type Batch = Vec<(WorkspaceId, Option<GitStatus>)>;

enum Msg {
    SetRoots(Roots),
    /// Measure now (window focus), still subject to `MIN_GAP`.
    Refresh(WorkspaceId),
    /// A path changed. The flag is set when it may be new (created or renamed in).
    Fs(PathBuf, bool),
    /// The poll timer fired (only used without a watcher).
    PollAll,
}

pub(crate) struct GitHub {
    tx: mpsc::Sender<Msg>,
    pub status: RefCell<HashMap<WorkspaceId, GitStatus>>,
    roots: RefCell<Roots>,
}

impl GitHub {
    /// The hub and the channel the main thread listens on.
    pub(crate) fn new() -> (Self, async_channel::Receiver<Batch>) {
        let (tx, rx) = mpsc::channel();
        let (out_tx, out_rx) = async_channel::unbounded();
        let events = tx.clone();
        std::thread::Builder::new()
            .name("xenon-git".into())
            .spawn(move || worker(rx, events, out_tx))
            .ok();
        (
            Self {
                tx,
                status: RefCell::default(),
                roots: RefCell::default(),
            },
            out_rx,
        )
    }

    pub(crate) fn dirt(&self, ws: WorkspaceId) -> DirtState {
        self.status
            .borrow()
            .get(&ws)
            .map_or(DirtState::Clean, |s| s.state)
    }

    pub(crate) fn status_of(&self, ws: WorkspaceId) -> Option<GitStatus> {
        self.status.borrow().get(&ws).cloned()
    }

    /// Tell the worker which folders to watch, when that set changed.
    pub(crate) fn set_roots(&self, roots: Roots) {
        if *self.roots.borrow() == roots {
            return;
        }
        let open: HashSet<WorkspaceId> = roots.iter().map(|(id, _)| *id).collect();
        self.status.borrow_mut().retain(|id, _| open.contains(id));
        *self.roots.borrow_mut() = roots.clone();
        let _ = self.tx.send(Msg::SetRoots(roots));
    }

    pub(crate) fn refresh(&self, ws: WorkspaceId) {
        let _ = self.tx.send(Msg::Refresh(ws));
    }
}

/// Apply finished measurements. Returns the workspaces whose status changed.
pub(crate) fn apply(hub: &GitHub, batch: Batch) -> Vec<WorkspaceId> {
    let open: HashSet<WorkspaceId> = hub.roots.borrow().iter().map(|(id, _)| *id).collect();
    let mut changed = Vec::new();
    let mut status = hub.status.borrow_mut();
    for (ws, next) in batch {
        if !open.contains(&ws) {
            continue;
        }
        match next {
            Some(next) => {
                if status.get(&ws) != Some(&next) {
                    status.insert(ws, next);
                    changed.push(ws);
                }
            }
            None => {
                if status.remove(&ws).is_some() {
                    changed.push(ws);
                }
            }
        }
    }
    changed
}

/// Receive batches on the main loop for as long as the app lives.
pub(crate) fn listen(app: &Rc<App>, rx: async_channel::Receiver<Batch>) {
    let weak: Weak<App> = Rc::downgrade(app);
    glib::spawn_future_local(async move {
        while let Ok(batch) = rx.recv().await {
            let Some(app) = weak.upgrade() else { break };
            app.git_measured(batch);
        }
    });
}

struct Worker {
    roots: Roots,
    watcher: Option<notify::RecommendedWatcher>,
    events: mpsc::Sender<Msg>,
    pending: HashSet<WorkspaceId>,
    /// Some root has no watch: fall back to slow polling.
    polling: bool,
    /// The watched folders of each root.
    sets: HashMap<WorkspaceId, DirSet>,
    /// Roots added but not yet scanned (done after their first status).
    unscanned: Vec<WorkspaceId>,
}

impl Worker {
    fn take(&mut self, msg: Msg) {
        match msg {
            Msg::SetRoots(next) => {
                self.unwatch_removed(&next);
                let old: HashSet<WorkspaceId> = self.roots.iter().map(|(id, _)| *id).collect();
                let added: Vec<WorkspaceId> = next
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| !old.contains(id))
                    .collect();
                self.unscanned.extend(&added);
                self.pending.extend(added);
                self.roots = next;
            }
            Msg::Refresh(ws) => {
                self.pending.insert(ws);
            }
            Msg::PollAll => {
                self.pending.extend(self.roots.iter().map(|(id, _)| *id));
            }
            Msg::Fs(path, new) => {
                if let Some(ws) = xenon_git::watch::owner_of(&self.roots, &path) {
                    self.pending.insert(ws);
                    self.follow(ws, &path, new);
                }
            }
        }
    }

    /// Keep the watched folders in step with one event: forget a removed
    /// folder, watch a newly created one (and only its own subtree).
    fn follow(&mut self, ws: WorkspaceId, path: &std::path::Path, new: bool) {
        let (Some(set), Some(watcher)) = (self.sets.get_mut(&ws), self.watcher.as_mut()) else {
            return;
        };
        if !new {
            set.shrink(path);
            return;
        }
        for dir in set.grow(path) {
            if let Err(error) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
                eprintln!("[xg] git: cannot watch {}: {error}", dir.display());
                set.full = true;
                self.polling = true;
                break;
            }
        }
    }

    /// Drop the watches of roots that closed.
    fn unwatch_removed(&mut self, next: &Roots) {
        let keep: HashSet<WorkspaceId> = next.iter().map(|(id, _)| *id).collect();
        let gone: Vec<WorkspaceId> = self
            .sets
            .keys()
            .copied()
            .filter(|id| !keep.contains(id))
            .collect();
        for id in gone {
            if let (Some(set), Some(w)) = (self.sets.remove(&id), self.watcher.as_mut()) {
                for dir in set.dirs() {
                    let _ = w.unwatch(dir);
                }
            }
        }
        self.unscanned.retain(|id| keep.contains(id));
    }

    /// Scan and watch the roots added since the last call. Runs after their
    /// first status was published, so a slow walk never delays the sidebar.
    fn watch_new_roots(&mut self) {
        if self.watcher.is_none() {
            let tx = self.events.clone();
            self.watcher =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    let Ok(event) = res else { return };
                    let new = matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_))
                    );
                    for path in event.paths {
                        if !xenon_git::watch::is_noise(&path) {
                            let _ = tx.send(Msg::Fs(path, new));
                        }
                    }
                })
                .map_err(|error| {
                    eprintln!("[xg] git: no file watcher ({error}); polling every 10s")
                })
                .ok();
        }
        let Some(watcher) = self.watcher.as_mut() else {
            self.unscanned.clear();
            self.polling = true;
            return;
        };
        for ws in std::mem::take(&mut self.unscanned) {
            let Some((_, root)) = self.roots.iter().find(|(id, _)| *id == ws) else {
                continue;
            };
            let mut set = DirSet::scan(root);
            for dir in set.dirs() {
                if let Err(error) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                    eprintln!("[xg] git: cannot watch {}: {error}", dir.display());
                    self.polling = true;
                    break;
                }
            }
            if set.full {
                eprintln!(
                    "[xg] git: {} has too many folders to watch; polling",
                    root.display()
                );
                self.polling = true;
            }
            if self.polling {
                set.full = true;
            }
            self.sets.insert(ws, set);
        }
    }
}

fn worker(rx: mpsc::Receiver<Msg>, events: mpsc::Sender<Msg>, out: async_channel::Sender<Batch>) {
    let mut w = Worker {
        roots: Vec::new(),
        watcher: None,
        events,
        pending: HashSet::new(),
        polling: false,
        sets: HashMap::new(),
        unscanned: Vec::new(),
    };
    let mut last: HashMap<WorkspaceId, Instant> = HashMap::new();
    let mut carry: Vec<Msg> = Vec::new();
    loop {
        let first = match carry.pop() {
            Some(msg) => msg,
            None if w.polling => match rx.recv_timeout(POLL) {
                Ok(msg) => msg,
                Err(mpsc::RecvTimeoutError::Timeout) => Msg::PollAll,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(msg) => msg,
                Err(_) => return,
            },
        };
        w.take(first);
        for msg in carry.drain(..) {
            w.take(msg);
        }
        // Debounce: keep collecting until things go quiet.
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(msg) => w.take(msg),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        // Rate limit: wait out what is left of the gap, then take what came.
        let wait = w
            .pending
            .iter()
            .filter_map(|ws| last.get(ws))
            .map(|at| MIN_GAP.saturating_sub(at.elapsed()))
            .max()
            .unwrap_or_default();
        if !wait.is_zero() {
            std::thread::sleep(wait);
            while let Ok(msg) = rx.try_recv() {
                w.take(msg);
            }
        }
        let mut batch = Batch::new();
        for ws in std::mem::take(&mut w.pending) {
            let Some((_, root)) = w.roots.iter().find(|(id, _)| *id == ws) else {
                continue;
            };
            last.insert(ws, Instant::now());
            batch.push((ws, xenon_git::measure::measure(root)));
        }
        last.retain(|ws, _| w.roots.iter().any(|(id, _)| id == ws));
        if !batch.is_empty() && out.send_blocking(batch).is_err() {
            return;
        }
        w.watch_new_roots();
        // Our own git calls can touch `.git`; that is not a change to act on.
        while let Ok(msg) = rx.try_recv() {
            if !matches!(&msg, Msg::Fs(path, _) if xenon_git::watch::is_git_meta(path)) {
                carry.push(msg);
            }
        }
    }
}
