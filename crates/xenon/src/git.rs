//! Git state for every open workspace, measured off the main thread.
//!
//! One worker thread runs `xenon_git::measure`. It sleeps in a blocking receive.
//! Changes reach it from three places: `gio` file monitors on each repository's
//! `.git` (commits, checkouts, staging), a window-focus refresh, and a slow
//! timer for the active workspace that catches edits to working-tree files (gio
//! cannot watch a tree recursively, and a monitor per folder would cost memory
//! on big trees). Events are debounced, and one workspace is measured at most
//! once per `MIN_GAP`, so a build or a save burst makes a few git calls, never
//! one per event. The main thread only receives finished statuses.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use xenon_core::WorkspaceId;
use xenon_git::status::{DirtState, GitStatus};

use crate::app::App;

/// Quiet time after the last event before git runs.
const DEBOUNCE: Duration = Duration::from_millis(400);
/// Least time between two measurements of one workspace.
const MIN_GAP: Duration = Duration::from_millis(1500);
/// While the window is focused, the active workspace is measured this often.
const POLL: Duration = Duration::from_secs(5);

type Roots = Vec<(WorkspaceId, PathBuf)>;
/// `None` status: the folder is not a git work tree (or was closed).
type Batch = Vec<(WorkspaceId, Option<GitStatus>)>;

enum Msg {
    SetRoots(Roots),
    /// Measure now (window focus), still subject to `MIN_GAP`.
    Refresh(WorkspaceId),
    /// Something in the repository's `.git` changed. Measuring touches `.git`
    /// itself, so these arriving during a measurement are dropped.
    Meta(WorkspaceId),
}

pub(crate) struct GitHub {
    tx: mpsc::Sender<Msg>,
    pub status: RefCell<HashMap<WorkspaceId, GitStatus>>,
    roots: RefCell<Roots>,
    monitors: RefCell<HashMap<WorkspaceId, Vec<gio::FileMonitor>>>,
}

impl GitHub {
    /// The hub and the channel the main thread listens on.
    pub(crate) fn new() -> (Self, async_channel::Receiver<Batch>) {
        let (tx, rx) = mpsc::channel();
        let (out_tx, out_rx) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("xenon-git".into())
            .spawn(move || worker(rx, out_tx))
            .ok();
        (
            Self {
                tx,
                status: RefCell::default(),
                roots: RefCell::default(),
                monitors: RefCell::default(),
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
        let mut monitors = self.monitors.borrow_mut();
        monitors.retain(|id, _| open.contains(id));
        for (id, root) in &roots {
            monitors
                .entry(*id)
                .or_insert_with(|| self.watch_repo(*id, root));
        }
        drop(monitors);
        let _ = self.tx.send(Msg::SetRoots(roots));
    }

    /// Monitors on `.git` (index, HEAD, packed refs) and `.git/refs/heads`.
    /// A linked worktree has a `.git` file instead; it gets no monitors and
    /// relies on the timer and the focus refresh.
    fn watch_repo(&self, ws: WorkspaceId, root: &std::path::Path) -> Vec<gio::FileMonitor> {
        let git = root.join(".git");
        let heads = git.join("refs/heads");
        [
            (
                git,
                &["index", "HEAD", "MERGE_HEAD", "FETCH_HEAD", "packed-refs"][..],
            ),
            (heads, &[][..]),
        ]
        .into_iter()
        .filter_map(|(dir, names)| {
            let monitor = gio::File::for_path(dir)
                .monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
                .ok()?;
            let tx = self.tx.clone();
            monitor.connect_changed(move |_, file, _, _| {
                let Some(name) = file.basename() else { return };
                let name = name.to_string_lossy();
                let wanted = if names.is_empty() {
                    !name.ends_with(".lock")
                } else {
                    names.contains(&&*name)
                };
                if wanted {
                    let _ = tx.send(Msg::Meta(ws));
                }
            });
            Some(monitor)
        })
        .collect()
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
    let timer = weak.clone();
    glib::timeout_add_local(POLL, move || {
        let Some(app) = timer.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if app.win.is_active() {
            app.git_refresh_active();
        }
        glib::ControlFlow::Continue
    });
    glib::spawn_future_local(async move {
        while let Ok(batch) = rx.recv().await {
            let Some(app) = weak.upgrade() else { break };
            app.git_measured(batch);
        }
    });
}

struct Worker {
    roots: Roots,
    pending: HashSet<WorkspaceId>,
}

impl Worker {
    fn take(&mut self, msg: Msg) {
        match msg {
            Msg::SetRoots(next) => {
                let old: HashSet<WorkspaceId> = self.roots.iter().map(|(id, _)| *id).collect();
                let added: Vec<WorkspaceId> = next
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| !old.contains(id))
                    .collect();
                self.pending.extend(added);
                self.roots = next;
            }
            Msg::Refresh(ws) | Msg::Meta(ws) => {
                self.pending.insert(ws);
            }
        }
    }
}

fn worker(rx: mpsc::Receiver<Msg>, out: async_channel::Sender<Batch>) {
    let mut w = Worker {
        roots: Vec::new(),
        pending: HashSet::new(),
    };
    let mut last: HashMap<WorkspaceId, Instant> = HashMap::new();
    let mut carry: Vec<Msg> = Vec::new();
    loop {
        let first = match carry.pop() {
            Some(msg) => msg,
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
        // Our own git calls can touch `.git`; that is not a change to act on.
        while let Ok(msg) = rx.try_recv() {
            if !matches!(msg, Msg::Meta(_)) {
                carry.push(msg);
            }
        }
    }
}
