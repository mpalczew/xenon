//! FS-event driven git dirtiness for the sidebar.
//!
//! Watches each open workspace root (recursive). Events are debounced, then
//! only the affected workspaces are re-measured via `git_dirt::measure`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use super::*;
use crate::git_dirt::{self, GitDirt};

const DEBOUNCE: Duration = Duration::from_millis(400);

impl XeroApp {
    pub(super) fn start_git_dirt_watch(&mut self, cx: &mut Context<Self>) {
        self.restart_git_dirt_watch(cx);
    }

    /// Rebuild watches for the current open workspaces (add/close/reopen).
    pub(super) fn restart_git_dirt_watch(&mut self, cx: &mut Context<Self>) {
        let roots = self
            .registry
            .workspaces
            .iter()
            .map(|w| (w.id, w.root.clone()))
            .collect::<Vec<_>>();
        // Dropping the previous task cancels it and tears down its watcher.
        self._git_dirt_task = Some(cx.spawn(async move |app, cx| {
            watch_git_dirt(app, cx, roots).await;
        }));
    }

    pub(crate) fn workspace_dirt(&self, id: WorkspaceId) -> Option<GitDirt> {
        self.git_dirt.get(&id).copied()
    }
}

async fn watch_git_dirt(
    app: gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    roots: Vec<(WorkspaceId, PathBuf)>,
) {
    let snapshot = measure_all(&roots);
    if apply_full(&app, cx, snapshot).is_err() {
        return;
    }
    let Some(rx) = spawn_watcher(&roots) else {
        return;
    };
    event_loop(&app, cx, &roots, rx).await;
}

fn spawn_watcher(
    roots: &[(WorkspaceId, PathBuf)],
) -> Option<(async_channel::Receiver<PathBuf>, notify::RecommendedWatcher)> {
    let (tx, rx) = async_channel::unbounded();
    let mut watcher = match notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            for path in event_paths(event) {
                let _ = tx.try_send(path);
            }
        }
    }) {
        Ok(w) => w,
        Err(error) => {
            log::error!("git dirt watcher failed to start: {error}");
            return None;
        }
    };
    for (_, root) in roots {
        if let Err(error) = watcher.watch(root, RecursiveMode::Recursive) {
            log::warn!("git dirt: cannot watch {}: {error}", root.display());
        }
    }
    Some((rx, watcher))
}

async fn event_loop(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    roots: &[(WorkspaceId, PathBuf)],
    rx: (async_channel::Receiver<PathBuf>, notify::RecommendedWatcher),
) {
    let (rx, _watcher) = rx;
    loop {
        let Ok(first) = rx.recv().await else {
            break;
        };
        let mut pending = HashSet::new();
        mark_workspace(roots, &first, &mut pending);
        cx.background_executor().timer(DEBOUNCE).await;
        while let Ok(path) = rx.try_recv() {
            mark_workspace(roots, &path, &mut pending);
        }
        if pending.is_empty() {
            continue;
        }
        let batch: Vec<_> = roots
            .iter()
            .filter(|(id, _)| pending.contains(id))
            .cloned()
            .collect();
        let updates = cx
            .background_executor()
            .spawn(async move { measure_all(&batch) })
            .await;
        if apply_partial(app, cx, &pending, updates).is_err() {
            break;
        }
    }
}

fn event_paths(event: notify::Event) -> impl Iterator<Item = PathBuf> {
    event.paths.into_iter().filter(|path| !is_noise(path))
}

/// Drop high-churn paths that never affect git dirt for normal projects.
fn is_noise(path: &Path) -> bool {
    if is_git_meta(path) {
        return false;
    }
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("target" | "node_modules" | "dist" | ".next" | "__pycache__" | ".git")
        )
    })
}

/// Index/HEAD/refs under `.git` affect dirtiness; object packs do not.
fn is_git_meta(path: &Path) -> bool {
    let mut parts = path.components().filter_map(|c| c.as_os_str().to_str());
    while let Some(part) = parts.next() {
        if part != ".git" {
            continue;
        }
        return matches!(
            parts.next(),
            None | Some("index" | "HEAD" | "FETCH_HEAD" | "MERGE_HEAD" | "refs" | "COMMIT_EDITMSG")
        );
    }
    false
}

fn mark_workspace(
    roots: &[(WorkspaceId, PathBuf)],
    path: &Path,
    pending: &mut HashSet<WorkspaceId>,
) {
    let mut best: Option<(usize, WorkspaceId)> = None;
    for (id, root) in roots {
        if path.starts_with(root) {
            let len = root.as_os_str().len();
            if best.is_none_or(|(prev, _)| len > prev) {
                best = Some((len, *id));
            }
        }
    }
    if let Some((_, id)) = best {
        pending.insert(id);
    }
}

fn measure_all(roots: &[(WorkspaceId, PathBuf)]) -> HashMap<WorkspaceId, GitDirt> {
    let mut map = HashMap::new();
    for (id, root) in roots {
        if let Some(dirt) = git_dirt::measure(root)
            && !dirt.is_clean()
        {
            map.insert(*id, dirt);
        }
    }
    map
}

fn apply_full(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    snapshot: HashMap<WorkspaceId, GitDirt>,
) -> Result<(), ()> {
    app.update(cx, |app, cx| {
        if app.git_dirt != snapshot {
            app.git_dirt = snapshot;
            cx.notify();
        }
    })
    .map_err(|_| ())
}

fn apply_partial(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    measured: &HashSet<WorkspaceId>,
    updates: HashMap<WorkspaceId, GitDirt>,
) -> Result<(), ()> {
    app.update(cx, |app, cx| {
        let mut changed = false;
        for &id in measured {
            match updates.get(&id) {
                Some(&dirt) => {
                    if app.git_dirt.insert(id, dirt) != Some(dirt) {
                        changed = true;
                    }
                }
                None => {
                    if app.git_dirt.remove(&id).is_some() {
                        changed = true;
                    }
                }
            }
        }
        if changed {
            cx.notify();
        }
    })
    .map_err(|_| ())
}
