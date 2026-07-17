//! Open Workspace palette: known + discovered roots, never opens missing dirs.
//!
//! Directory discovery runs on a background thread so large `~/src` trees cannot
//! beach-ball the UI on each keystroke. Chrome: `crate::palette`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Task,
    Window, div,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xero_core::WorkspaceId;

use crate::impl_palette_query_input;
use crate::palette::{
    DetailRow, PaletteLayout, ScrollResults, detail_row, hint_row_with_action, input_registrar,
    panel, query_row, scrim, scroll_results,
};
use crate::workspace_discover::{
    DiscoverQuery, FoundRoot, MatchQuality, discover, expand_user_path, parse_discover_query,
    path_is_dir, ranking_needle, resolve_existing_dir, same_root,
};

const DISCOVER_DEBOUNCE: Duration = Duration::from_millis(60);

/// One row the user can confirm (or see as missing).
#[derive(Clone, Debug)]
pub enum WorkspaceCandidate {
    Open {
        id: WorkspaceId,
        name: String,
        root: PathBuf,
    },
    Closed {
        id: WorkspaceId,
        name: String,
        root: PathBuf,
        missing: bool,
    },
    /// Typed or discovered existing directory.
    Path { root: PathBuf, found: bool },
}

impl WorkspaceCandidate {
    pub fn name(&self) -> String {
        match self {
            Self::Open { name, .. } | Self::Closed { name, .. } => name.clone(),
            Self::Path { root, .. } => root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        }
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::Open { root, .. } | Self::Closed { root, .. } | Self::Path { root, .. } => root,
        }
    }

    pub fn selectable(&self) -> bool {
        match self {
            Self::Closed { missing: true, .. } => false,
            Self::Open { root, .. }
            | Self::Closed {
                root,
                missing: false,
                ..
            } => path_is_dir(root),
            Self::Path { root, .. } => path_is_dir(root),
        }
    }

    fn haystack(&self) -> String {
        format!("{} {}", self.name(), self.root().display())
    }

    fn badge(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open",
            Self::Closed { missing: true, .. } => "missing",
            Self::Closed { .. } => "closed",
            Self::Path { found: true, .. } => "found",
            Self::Path { .. } => "path",
        }
    }

    fn rank_tier(&self) -> u8 {
        match self {
            Self::Path { found: false, .. } => 0,
            Self::Open { .. } => 1,
            Self::Path { found: true, .. } => 2,
            Self::Closed { missing: false, .. } => 3,
            Self::Closed { missing: true, .. } => 4,
        }
    }
}

pub enum WorkspacePickerEvent {
    Open(WorkspaceCandidate),
    Browse,
    Dismissed,
}

pub struct WorkspacePickerView {
    known: Vec<WorkspaceCandidate>,
    query: String,
    results: Vec<WorkspaceCandidate>,
    /// Latest background-discovery hits for the current query.
    discovered: Vec<FoundRoot>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
    matcher: Matcher,
    scroll: ScrollHandle,
    discover_gen: u64,
    _discover_task: Option<Task<()>>,
}

impl EventEmitter<WorkspacePickerEvent> for WorkspacePickerView {}

impl WorkspacePickerView {
    pub fn new(known: Vec<WorkspaceCandidate>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            known,
            query: String::new(),
            results: Vec::new(),
            discovered: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
            discover_gen: 0,
            _discover_task: None,
        };
        view.refilter();
        view
    }

    fn refilter(&mut self) {
        let mut results = self.filter_known();
        self.merge_discovered(&mut results);
        self.add_exact_path(&mut results);
        let needle = ranking_needle(&self.query);
        results.sort_by(|a, b| {
            a.rank_tier()
                .cmp(&b.rank_tier())
                .then_with(|| {
                    basename_rank(a, needle.as_deref()).cmp(&basename_rank(b, needle.as_deref()))
                })
                .then_with(|| path_component_count(a.root()).cmp(&path_component_count(b.root())))
                .then_with(|| a.root().as_os_str().len().cmp(&b.root().as_os_str().len()))
                .then_with(|| a.name().cmp(&b.name()))
        });
        self.results = results;
        self.selected = self
            .results
            .iter()
            .position(|c| c.selectable())
            .unwrap_or(0);
    }

    fn filter_known(&mut self) -> Vec<WorkspaceCandidate> {
        if self.query.is_empty() {
            return self.known.clone();
        }
        let haystacks: Vec<String> = self.known.iter().map(|c| c.haystack()).collect();
        crate::palette::fuzzy_index_order(&haystacks, &self.query, &mut self.matcher)
            .into_iter()
            .map(|i| self.known[i].clone())
            .collect()
    }

    fn add_exact_path(&self, results: &mut Vec<WorkspaceCandidate>) {
        // Cheap existence check first; canonicalize only on confirm.
        let Some(expanded) = expand_user_path(&self.query) else {
            return;
        };
        if !expanded.is_dir() {
            return;
        }
        if results
            .iter()
            .any(|c| c.root() == expanded.as_path() || same_root(c.root(), &expanded))
        {
            return;
        }
        results.insert(
            0,
            WorkspaceCandidate::Path {
                root: expanded,
                found: false,
            },
        );
    }

    fn merge_discovered(&self, results: &mut Vec<WorkspaceCandidate>) {
        for found in &self.discovered {
            if results
                .iter()
                .any(|c| c.root() == found.path.as_path() || same_root(c.root(), &found.path))
            {
                continue;
            }
            results.push(WorkspaceCandidate::Path {
                root: found.path.clone(),
                found: true,
            });
        }
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.discovered.clear();
        self.refilter();
        self.kick_discover(cx);
        cx.notify();
    }

    /// Walk the FS off the UI thread; merge when still matching the latest query.
    fn kick_discover(&mut self, cx: &mut Context<Self>) {
        let Some(parsed) = parse_discover_query(&self.query) else {
            self._discover_task = None;
            return;
        };
        if matches!(parsed, DiscoverQuery::Exact(_)) {
            self._discover_task = None;
            return;
        }
        self.discover_gen = self.discover_gen.wrapping_add(1);
        let token = self.discover_gen;
        let query_snapshot = self.query.clone();
        let q = parsed;
        self._discover_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DISCOVER_DEBOUNCE).await;
            let still = this
                .update(cx, |this, _| {
                    this.discover_gen == token && this.query == query_snapshot
                })
                .unwrap_or(false);
            if !still {
                return;
            }
            let found = cx
                .background_executor()
                .spawn(async move { discover(&q) })
                .await;
            this.update(cx, |this, cx| {
                if this.discover_gen != token || this.query != query_snapshot {
                    return;
                }
                this.discovered = found;
                this.refilter();
                cx.notify();
            })
            .ok();
        }));
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let selectable: Vec<usize> = self
            .results
            .iter()
            .enumerate()
            .filter(|(_, c)| c.selectable())
            .map(|(i, _)| i)
            .collect();
        if selectable.is_empty() {
            return;
        }
        let cur = selectable
            .iter()
            .position(|&i| i == self.selected)
            .unwrap_or(0);
        let next = (cur as isize + delta).clamp(0, (selectable.len() - 1) as isize) as usize;
        self.selected = selectable[next];
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(candidate) = self.results.get(self.selected).cloned() {
            if !candidate.selectable() {
                return;
            }
            if let Some(root) = resolve_existing_dir(candidate.root()) {
                let candidate = match candidate {
                    WorkspaceCandidate::Open { id, name, .. } => {
                        WorkspaceCandidate::Open { id, name, root }
                    }
                    WorkspaceCandidate::Closed { id, name, .. } => WorkspaceCandidate::Closed {
                        id,
                        name,
                        root,
                        missing: false,
                    },
                    WorkspaceCandidate::Path { found, .. } => {
                        WorkspaceCandidate::Path { root, found }
                    }
                };
                cx.emit(WorkspacePickerEvent::Open(candidate));
            }
            return;
        }
        if let Some(root) = expand_user_path(&self.query).and_then(|p| resolve_existing_dir(&p)) {
            cx.emit(WorkspacePickerEvent::Open(WorkspaceCandidate::Path {
                root,
                found: false,
            }));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(WorkspacePickerEvent::Dismissed),
            "enter" => self.confirm(cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "backspace" => {
                let mut query = self.query.clone();
                query.pop();
                self.set_query(query, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Focusable for WorkspacePickerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WorkspacePickerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let empty = if self.query.is_empty() {
            "No workspaces — type a path, name, or ~/src name"
        } else {
            "No match — try ~/src name or Browse…"
        };
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                let selectable = cand.selectable();
                let selected = i == self.selected && selectable;
                let mut row = detail_row(
                    ("workspace-row", i),
                    DetailRow {
                        title: cand.name(),
                        detail: cand.badge().to_string(),
                        selected,
                        selectable,
                        subtitle: Some(cand.root().display().to_string()),
                    },
                    &colors,
                );
                if selectable {
                    row = row.on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.confirm(cx);
                    }));
                }
                row.into_any_element()
            })
            .collect();
        let browse = div()
            .id("workspace-picker-browse-btn")
            .cursor_pointer()
            .hover(|s| s.text_color(colors.text))
            .child("Browse… ⌘⇧O")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkspacePickerEvent::Browse)))
            .into_any_element();

        scrim("workspace-picker-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkspacePickerEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("WorkspacePicker")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(
                        query_row(&self.query, "Open workspace…", true, &colors).into_any_element(),
                    )
                    .child(scroll_results(ScrollResults {
                        list_id: "workspace-picker-results",
                        empty_message: empty,
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }))
                    .child(
                        hint_row_with_action(
                            "↵ open  ·  ~/src name  ·  esc  ·  missing not selectable",
                            browse,
                            &colors,
                        )
                        .into_any_element(),
                    ),
            )
    }
}

impl_palette_query_input!(WorkspacePickerView);

/// Lower is better. Exact basename match to the discovery needle ranks first.
fn basename_rank(c: &WorkspaceCandidate, needle: Option<&str>) -> u8 {
    let Some(n) = needle else {
        return 1;
    };
    match MatchQuality::of_basename(&c.name(), n) {
        Some(MatchQuality::Exact) => 0,
        Some(MatchQuality::Prefix) => 1,
        Some(MatchQuality::Contains) => 2,
        None => 3,
    }
}

fn path_component_count(path: &Path) -> usize {
    path.components().count()
}
