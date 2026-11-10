use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use xenon_core::WorkspaceId;
use xenon_git::status::{DirtState, GitStatus};

use crate::app::{App, defer};

const DEBOUNCE: Duration = Duration::from_millis(400);
const MIN_GAP: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_secs(5);

type Roots = Vec<(WorkspaceId, PathBuf)>;
type Batch = Vec<(WorkspaceId, Option<GitStatus>)>;

pub(crate) struct GitHub {
    app: Weak<App>,
    pub status: RefCell<HashMap<WorkspaceId, GitStatus>>,
    roots: RefCell<Roots>,
    monitors: RefCell<HashMap<WorkspaceId, Vec<gio::FileMonitor>>>,
    armed: RefCell<HashSet<WorkspaceId>>,
    running: RefCell<HashSet<WorkspaceId>>,
    last: RefCell<HashMap<WorkspaceId, Instant>>,
}

impl GitHub {
    pub(crate) fn new(app: Weak<App>) -> Self {
        Self {
            app,
            status: RefCell::default(),
            roots: RefCell::default(),
            monitors: RefCell::default(),
            armed: RefCell::default(),
            running: RefCell::default(),
            last: RefCell::default(),
        }
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

    pub(crate) fn set_roots(&self, roots: Roots) {
        if *self.roots.borrow() == roots {
            return;
        }
        let open: HashSet<WorkspaceId> = roots.iter().map(|(id, _)| *id).collect();
        let added: Vec<WorkspaceId> = {
            let old = self.roots.borrow();
            open.iter()
                .copied()
                .filter(|id| !old.iter().any(|(o, _)| o == id))
                .collect()
        };
        self.status.borrow_mut().retain(|id, _| open.contains(id));
        self.last.borrow_mut().retain(|id, _| open.contains(id));
        *self.roots.borrow_mut() = roots.clone();
        let mut monitors = self.monitors.borrow_mut();
        monitors.retain(|id, _| open.contains(id));
        for (id, root) in &roots {
            monitors
                .entry(*id)
                .or_insert_with(|| self.watch_repo(*id, root));
        }
        drop(monitors);
        for ws in added {
            self.refresh(ws);
        }
    }

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
            let app = self.app.clone();
            monitor.connect_changed(move |_, file, _, _| {
                let Some(name) = file.basename() else { return };
                let name = name.to_string_lossy();
                let wanted = if names.is_empty() {
                    !name.ends_with(".lock")
                } else {
                    names.contains(&&*name)
                };
                if wanted {
                    defer(&app, move |app| {
                        app.git.meta(ws);
                    });
                }
            });
            Some(monitor)
        })
        .collect()
    }

    pub(crate) fn refresh(&self, ws: WorkspaceId) {
        self.arm(ws, DEBOUNCE);
    }

    fn meta(&self, ws: WorkspaceId) {
        if !self.running.borrow().contains(&ws) {
            self.arm(ws, DEBOUNCE);
        }
    }

    fn arm(&self, ws: WorkspaceId, delay: Duration) {
        if !self.armed.borrow_mut().insert(ws) {
            return;
        }
        let weak = self.app.clone();
        glib::timeout_add_local_once(delay, move || {
            if let Some(app) = weak.upgrade() {
                app.git.fire(&app, ws);
            }
        });
    }

    fn fire(&self, app: &Rc<App>, ws: WorkspaceId) {
        self.armed.borrow_mut().remove(&ws);
        let Some(root) = self
            .roots
            .borrow()
            .iter()
            .find(|(id, _)| *id == ws)
            .map(|(_, root)| root.clone())
        else {
            return;
        };
        if self.running.borrow().contains(&ws) {
            return self.arm(ws, DEBOUNCE);
        }
        let wait = self
            .last
            .borrow()
            .get(&ws)
            .map_or(Duration::ZERO, |at| MIN_GAP.saturating_sub(at.elapsed()));
        if !wait.is_zero() {
            return self.arm(ws, wait);
        }
        self.last.borrow_mut().insert(ws, Instant::now());
        self.running.borrow_mut().insert(ws);
        let weak = Rc::downgrade(app);
        glib::spawn_future_local(async move {
            let status = gio::spawn_blocking(move || xenon_git::measure::measure(&root))
                .await
                .ok();
            let Some(app) = weak.upgrade() else { return };
            if let Some(status) = status {
                app.git_measured(vec![(ws, status)]);
            }
            // The measure itself touches .git; let those queued monitor events drain first.
            glib::idle_add_local_once(move || {
                app.git.running.borrow_mut().remove(&ws);
            });
        });
    }
}

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

pub(crate) fn listen(app: &Rc<App>) {
    let timer = Rc::downgrade(app);
    glib::timeout_add_local(POLL, move || {
        let Some(app) = timer.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if app.win.is_active() {
            app.git_refresh_active();
        }
        glib::ControlFlow::Continue
    });
}
