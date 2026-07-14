//! FS-event driven git dirtiness for the sidebar.
//!
//! Watches each open workspace root (recursive). Events are debounced, then
//! only the affected workspaces are re-measured via `git_dirt::measure`.
//!
//! The watcher is long-lived: open/close/reopen only diffs roots (unwatch /
//! watch) instead of tearing down FSEvents. Dropping a recursive watcher on
//! the UI thread freezes the app on large trees.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use super::*;
use crate::git_dirt::{self, GitDirt};

const DEBOUNCE: Duration = Duration::from_millis(400);

/// Messages into the long-lived git-dirt task.
pub(super) enum DirtMsg {
    /// Filesystem path that changed (from notify).
    Fs(PathBuf),
    /// Replace the set of watched workspace roots.
    SetRoots(Vec<(WorkspaceId, PathBuf)>),
}

impl XeroApp {
    pub(super) fn start_git_dirt_watch(&mut self, cx: &mut Context<Self>) {
        self.restart_git_dirt_watch(cx);
    }

    /// Sync watches with the current open workspaces (add/close/reopen).
    ///
    /// Prefer updating the existing task; only spawn once. Replacing the task
    /// drops the recursive FSEvents watcher on the UI thread and freezes.
    pub(super) fn restart_git_dirt_watch(&mut self, cx: &mut Context<Self>) {
        let roots = self
            .registry
            .workspaces
            .iter()
            .map(|w| (w.id, w.root.clone()))
            .collect::<Vec<_>>();
        if let Some(tx) = &self.git_dirt_tx
            && tx.try_send(DirtMsg::SetRoots(roots.clone())).is_ok()
        {
            return;
        }
        let (tx, rx) = async_channel::unbounded();
        self.git_dirt_tx = Some(tx.clone());
        self._git_dirt_task = Some(cx.spawn(async move |app, cx| {
            watch_git_dirt(app, cx, roots, tx, rx).await;
        }));
    }

    pub(crate) fn workspace_dirt(&self, id: WorkspaceId) -> Option<GitDirt> {
        self.git_dirt.get(&id).copied()
    }
}

async fn watch_git_dirt(
    app: gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    mut roots: Vec<(WorkspaceId, PathBuf)>,
    tx: async_channel::Sender<DirtMsg>,
    rx: async_channel::Receiver<DirtMsg>,
) {
    // Measure and install the watcher off the UI thread: both are sync I/O.
    let measured_roots = roots.clone();
    let snapshot = cx
        .background_executor()
        .spawn(async move { measure_all(&measured_roots) })
        .await;
    if apply_full(&app, cx, snapshot).is_err() {
        return;
    }
    let watch_roots = roots.clone();
    let event_tx = tx;
    let mut watcher = cx
        .background_executor()
        .spawn(async move { spawn_watcher(&watch_roots, event_tx) })
        .await;
    if watcher.is_none() {
        return;
    }

    let mut pending_roots: Option<Vec<(WorkspaceId, PathBuf)>> = None;
    loop {
        // Apply a SetRoots deferred from an FS debounce, if any.
        if let Some(new_roots) = pending_roots.take() {
            if apply_set_roots(&app, cx, &mut watcher, &mut roots, new_roots)
                .await
                .is_err()
            {
                break;
            }
            continue;
        }

        let Ok(msg) = rx.recv().await else {
            break;
        };
        match msg {
            DirtMsg::SetRoots(new_roots) => {
                if apply_set_roots(&app, cx, &mut watcher, &mut roots, new_roots)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            DirtMsg::Fs(path) => match handle_fs_burst(&app, cx, &roots, &rx, path).await {
                Ok(deferred) => pending_roots = deferred,
                Err(()) => break,
            },
        }
    }
}

async fn apply_set_roots(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    watcher: &mut Option<notify::RecommendedWatcher>,
    roots: &mut Vec<(WorkspaceId, PathBuf)>,
    new_roots: Vec<(WorkspaceId, PathBuf)>,
) -> Result<(), ()> {
    // Unwatch/watch is sync FSEvents work; never do it on the UI executor.
    let old = std::mem::take(roots);
    let old_ids: HashSet<WorkspaceId> = old.iter().map(|(id, _)| *id).collect();
    let mut owned = watcher.take();
    let (next_watcher, next_roots) = cx
        .background_executor()
        .spawn(async move {
            if let Some(w) = owned.as_mut() {
                sync_watches(w, &old, &new_roots);
            }
            (owned, new_roots)
        })
        .await;
    *watcher = next_watcher;
    *roots = next_roots;
    let open: HashSet<WorkspaceId> = roots.iter().map(|(id, _)| *id).collect();
    prune_closed(app, cx, &open)?;
    // Close-only: prune is enough. Remeasure only newly opened roots.
    let added: Vec<_> = roots
        .iter()
        .filter(|(id, _)| !old_ids.contains(id))
        .cloned()
        .collect();
    if added.is_empty() {
        return Ok(());
    }
    let measured: HashSet<WorkspaceId> = added.iter().map(|(id, _)| *id).collect();
    let updates = cx
        .background_executor()
        .spawn(async move { measure_all(&added) })
        .await;
    apply_partial(app, cx, &measured, updates)
}

/// Debounce FS events after the first path, then remeasure hit workspaces.
/// Returns a SetRoots that arrived mid-burst so the outer loop can apply it.
async fn handle_fs_burst(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    roots: &[(WorkspaceId, PathBuf)],
    rx: &async_channel::Receiver<DirtMsg>,
    first: PathBuf,
) -> Result<Option<Vec<(WorkspaceId, PathBuf)>>, ()> {
    let mut pending = HashSet::new();
    let mut files = HashSet::new();
    let mut deferred_roots = None;
    mark_workspace(roots, &first, &mut pending);
    if first.is_file() {
        files.insert(first);
    }
    cx.background_executor().timer(DEBOUNCE).await;
    while let Ok(msg) = rx.try_recv() {
        match msg {
            DirtMsg::Fs(path) => {
                mark_workspace(roots, &path, &mut pending);
                if path.is_file() {
                    files.insert(path);
                }
            }
            DirtMsg::SetRoots(new_roots) => {
                // Last SetRoots wins if several land during the debounce.
                deferred_roots = Some(new_roots);
            }
        }
    }
    if !files.is_empty() {
        let paths: Vec<_> = files.into_iter().collect();
        app.update(cx, |app, cx| app.sync_editors_for_paths(&paths, cx))
            .map_err(|_| ())?;
    }
    if !pending.is_empty() {
        let batch: Vec<_> = roots
            .iter()
            .filter(|(id, _)| pending.contains(id))
            .cloned()
            .collect();
        let updates = cx
            .background_executor()
            .spawn(async move { measure_all(&batch) })
            .await;
        apply_partial(app, cx, &pending, updates)?;
    }
    Ok(deferred_roots)
}

fn spawn_watcher(
    roots: &[(WorkspaceId, PathBuf)],
    tx: async_channel::Sender<DirtMsg>,
) -> Option<notify::RecommendedWatcher> {
    let mut watcher = match notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            for path in event_paths(event) {
                let _ = tx.try_send(DirtMsg::Fs(path));
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
    Some(watcher)
}

/// Unwatch removed roots and watch newly added ones. Does not recreate the watcher.
fn sync_watches(
    watcher: &mut notify::RecommendedWatcher,
    old: &[(WorkspaceId, PathBuf)],
    new: &[(WorkspaceId, PathBuf)],
) {
    let old_paths: HashSet<&Path> = old.iter().map(|(_, p)| p.as_path()).collect();
    let new_paths: HashSet<&Path> = new.iter().map(|(_, p)| p.as_path()).collect();
    for path in old_paths.difference(&new_paths) {
        if let Err(error) = watcher.unwatch(path) {
            log::warn!("git dirt: cannot unwatch {}: {error}", path.display());
        }
    }
    for path in new_paths.difference(&old_paths) {
        if let Err(error) = watcher.watch(path, RecursiveMode::Recursive) {
            log::warn!("git dirt: cannot watch {}: {error}", path.display());
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

fn prune_closed(
    app: &gpui::WeakEntity<XeroApp>,
    cx: &mut gpui::AsyncApp,
    open: &HashSet<WorkspaceId>,
) -> Result<(), ()> {
    app.update(cx, |app, cx| {
        let before = app.git_dirt.len();
        app.git_dirt.retain(|id, _| open.contains(id));
        if app.git_dirt.len() != before {
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
