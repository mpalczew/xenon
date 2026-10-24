//! Follow clicked paths and links (terminal cmd-click, markdown links): URLs go
//! to the system, file paths resolve against the document's folder, then the
//! workspace root, then the fuzzy file index.

use super::*;
use xenon_editor::LinkTarget;

/// Terminal path clicks that are view-first (not edit-first) should hand off
/// to the OS default app instead of opening a Xenon editor tab.
fn prefer_system_open(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("html" | "htm" | "pdf")
    )
}

impl XenonApp {
    /// Terminal or markdown click on a path: browser-native types go to the
    /// system default app; everything else opens as an editor tab, at `at`
    /// (zero-based row, column) when given.
    pub(super) fn open_terminal_path(
        &mut self,
        path: PathBuf,
        at: Option<(u32, u32)>,
        cx: &mut Context<Self>,
    ) {
        if !path.starts_with("/__xenon_ssh__") && prefer_system_open(&path) {
            cx.open_with_system(&path);
            return;
        }
        if let Err(error) = self.open_editor_at(path, true, at, cx) {
            self.show_toast(super::toasts::failed("Couldn’t open that file", error), cx);
        }
    }

    /// Fuzzy-match a path-like `token` against the workspace index: open the
    /// one file it names, else show the finder pre-filled with its basename.
    pub(super) fn resolve_clicked(
        &mut self,
        token: String,
        at: Option<(u32, u32)>,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.active.and_then(|id| self.workspace_root(id)) else {
            return;
        };
        self.reindex(root.clone(), false, cx);
        let basename = Path::new(&token)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| token.clone());
        let Some(index) = self.file_indexes.get(&root).cloned() else {
            self.deferred.pending_palette_query = Some(basename);
            cx.notify();
            return;
        };
        let recents = self
            .active
            .map(|id| self.recent_files_for(id))
            .unwrap_or_default();
        let results = Finder::new(index).query(&basename, &recents);
        let files = || results.iter().filter(|m| !m.is_dir);
        let exact: Vec<&std::path::PathBuf> = files()
            .filter(|m| {
                m.path.file_name().map(|n| n.to_string_lossy()) == Some(basename.as_str().into())
            })
            .map(|m| &m.path)
            .collect();
        let unique_file = || {
            files()
                .next()
                .filter(|_| files().count() == 1)
                .map(|m| &m.path)
        };
        let token_path = Path::new(token.trim_start_matches("./"));
        let mut by_suffix = exact.iter().filter(|rel| rel.ends_with(token_path));
        let suffix_match = by_suffix.next().filter(|_| by_suffix.next().is_none());
        if let Some(rel) = suffix_match.copied().filter(|_| exact.len() > 1) {
            self.open_terminal_path(root.join(rel), at, cx);
        } else if let Some(rel) = exact.first().copied().filter(|_| exact.len() == 1) {
            self.open_terminal_path(root.join(rel), at, cx);
        } else if exact.is_empty()
            && let Some(rel) = unique_file()
        {
            self.open_terminal_path(root.join(rel), at, cx);
        } else {
            self.deferred.pending_palette_query = Some(basename);
            cx.notify();
        }
    }

    pub(super) fn open_doc_link(
        &mut self,
        doc: &Path,
        target: &LinkTarget,
        cx: &mut Context<Self>,
    ) {
        let LinkTarget::Path { path, line, col } = target else {
            if let LinkTarget::Url(url) = target {
                cx.open_url(url);
            }
            return;
        };
        let at = line.map(|line| (line, col.unwrap_or(0)));
        let root = self.active.and_then(|id| self.workspace_root(id));
        let found = link_candidates(path, doc, root.as_deref())
            .into_iter()
            .find(|candidate| candidate.is_file());
        match found {
            Some(found) => self.open_terminal_path(found, at, cx),
            None => self.resolve_clicked(path.clone(), at, cx),
        }
    }
}

/// Where a link's path may live, most specific first. Absolute links also try
/// the workspace root, since repo docs often write `/docs/x.md` GitHub-style.
fn link_candidates(path: &str, doc: &Path, root: Option<&Path>) -> Vec<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        return std::env::home_dir()
            .map(|home| vec![home.join(rest)])
            .unwrap_or_default();
    }
    let written = Path::new(path);
    let mut candidates = Vec::new();
    if written.is_absolute() {
        candidates.push(written.to_path_buf());
        if let (Some(root), Ok(rel)) = (root, written.strip_prefix("/")) {
            candidates.push(root.join(rel));
        }
    } else {
        if let Some(dir) = doc.parent() {
            candidates.push(dir.join(written));
        }
        if let Some(root) = root {
            candidates.push(root.join(written));
        }
    }
    candidates
}
