//! Open Workspace palette: known + discovered roots, never opens missing dirs.
//!
//! Directory discovery runs on a background thread so large `~/src` trees cannot
//! beach-ball the UI on each keystroke. Chrome: `crate::palette`.
//!
//! Ranking: exact typed path → known workspaces (basename quality, then MRU) →
//! discovered dirs → missing. Known always beats discovery so a used workspace
//! like `personalfiles` wins over a random dir named `personal`.

mod candidate;
mod host;
mod rank;
mod render;
mod scope;
mod ssh;

pub use candidate::{WorkspaceCandidate, WorkspacePickerEvent};
#[cfg(feature = "visual-tests")]
pub(crate) use scope::Remote as VisualRemote;
pub(crate) use scope::host_candidates;

use std::time::Duration;

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Task, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_design_system::{
    PaletteInput, PaletteOverlay, QueryRow, palette_input, palette_overlay, query_hint_action,
    query_row,
};

use crate::palette::{PaletteLayout, ScrollResults, match_hits, reveal_selected, scroll_results};
use crate::workspace_discover::{
    DiscoverQuery, FoundRoot, discover, expand_user_path, parse_discover_query, ranking_needle,
    resolve_existing_dir, same_root,
};

use rank::{sort_candidates, sort_listing};
use scope::Scope;

const DISCOVER_DEBOUNCE: Duration = Duration::from_millis(60);

pub struct WorkspacePickerView {
    /// Virtual root of each SSH workspace in `known` → where it points.
    ssh_known: std::collections::HashMap<std::path::PathBuf, xenon_ssh::SshWorkspace>,
    scope: Scope,
    known: Vec<WorkspaceCandidate>,
    query: String,
    results: Vec<WorkspaceCandidate>,
    /// Latest background-discovery hits for the current query.
    discovered: Vec<FoundRoot>,
    selected: usize,
    focus: FocusHandle,
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    scroll: ScrollHandle,
    discover_gen: u64,
    _discover_task: Option<Task<()>>,
}

impl EventEmitter<WorkspacePickerEvent> for WorkspacePickerView {}

impl WorkspacePickerView {
    pub(crate) fn set_ssh_workspaces(
        &mut self,
        workspaces: std::collections::HashMap<std::path::PathBuf, xenon_ssh::SshWorkspace>,
    ) {
        self.ssh_known = workspaces;
    }

    #[cfg(feature = "visual-tests")]
    pub(crate) fn visual_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.query = query.into();
        self.input.update(cx, |input, cx| input.set_text(query, cx));
        self.refilter();
        cx.notify();
    }

    pub fn new(known: Vec<WorkspaceCandidate>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            xenon_design_system::TextInputView::new(
                xenon_design_system::TextInputConfig::single_line("Folder or ssh://host/path…")
                    .parent_navigation()
                    .appearance(xenon_design_system::TextInputAppearance::Palette),
                cx,
            )
        });
        input.update(cx, |input, cx| input.open(cx));
        let focus = input.read(cx).focus_handle();
        let input_sub = cx.subscribe(&input, |this, _, event, cx| match palette_input(event) {
            PaletteInput::Query(query) => this.set_query(query, cx),
            PaletteInput::Navigate {
                key,
                shift,
                platform,
            } => this.on_nav(&key, shift, platform, cx),
            PaletteInput::Ignore => {}
        });
        let mut view = Self {
            ssh_known: Default::default(),
            scope: Scope::Local,
            known,
            query: String::new(),
            results: Vec::new(),
            discovered: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
            discover_gen: 0,
            _discover_task: None,
        };
        view.refilter();
        view
    }

    fn refilter(&mut self) {
        let results = if self.scope.host().is_some() {
            self.host_results()
        } else {
            self.local_results()
        };
        self.results = results;
        self.selected = self
            .results
            .iter()
            .position(|c| c.selectable() || c.is_closed())
            .unwrap_or(0);
    }

    fn local_results(&mut self) -> Vec<WorkspaceCandidate> {
        let mut results = self.filter_known();
        self.merge_discovered(&mut results);
        self.add_exact_path(&mut results);
        let needle = ranking_needle(&self.query);
        sort_candidates(&mut results, needle.as_deref());
        results
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
        self.kick_remote(cx);
        self.refilter();
        self.kick_discover(cx);
        cx.notify();
    }

    /// Walk the FS off the UI thread; merge when still matching the latest query.
    fn kick_discover(&mut self, cx: &mut Context<Self>) {
        if self.scope.host().is_some() {
            return;
        }
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
        // Include missing closed rows so ⌘⌫ can target them.
        let navigable: Vec<usize> = self
            .results
            .iter()
            .enumerate()
            .filter(|(_, c)| c.selectable() || c.is_closed())
            .map(|(i, _)| i)
            .collect();
        if navigable.is_empty() {
            return;
        }
        let cur = navigable
            .iter()
            .position(|&i| i == self.selected)
            .unwrap_or(0);
        let next = (cur as isize + delta).clamp(0, (navigable.len() - 1) as isize) as usize;
        self.selected = navigable[next];
        reveal_selected(&self.scroll, self.selected);
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.query.trim().starts_with("ssh://") {
            cx.emit(WorkspacePickerEvent::Ssh(self.query.trim().to_string()));
            return;
        }
        if self.confirm_host_row(cx) {
            return;
        }
        if let Some(candidate) = self.results.get(self.selected).cloned() {
            if !candidate.selectable() {
                return;
            }
            if candidate.root().starts_with("/__xenon_ssh__") {
                cx.emit(WorkspacePickerEvent::Open(candidate));
                return;
            }
            if let Some(root) = resolve_existing_dir(candidate.root()) {
                let candidate = candidate.with_root(root);
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

    /// Remove selected closed workspace from history (⌘⌫ / delete).
    fn forget_selected(&mut self, cx: &mut Context<Self>) {
        let Some(candidate) = self.results.get(self.selected) else {
            return;
        };
        if !candidate.is_closed() {
            return;
        }
        let Some(id) = candidate.workspace_id() else {
            return;
        };
        self.known.retain(|c| c.workspace_id() != Some(id));
        self.refilter();
        // Keep selection in range after drop.
        if self.selected >= self.results.len() && !self.results.is_empty() {
            self.selected = self.results.len() - 1;
        }
        cx.emit(WorkspacePickerEvent::Forget(id));
        cx.notify();
    }

    fn on_nav(&mut self, key: &str, _shift: bool, platform: bool, cx: &mut Context<Self>) {
        match key {
            "escape" => cx.emit(WorkspacePickerEvent::Dismissed),
            "enter" => self.confirm(cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "backspace" if platform => self.forget_selected(cx),
            "backspace" => self.leave_host_if_empty(cx),
            "tab" => self.complete_selected(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if key == "delete" {
            self.forget_selected(cx);
            cx.stop_propagation();
            return;
        }
        self.on_nav(
            key,
            event.keystroke.modifiers.shift,
            event.keystroke.modifiers.secondary(),
            cx,
        );
    }
}

impl Focusable for WorkspacePickerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
