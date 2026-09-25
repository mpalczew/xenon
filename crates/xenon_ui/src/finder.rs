//! `FinderView`: the cmd-p fuzzy file palette. Emits `FinderEvent` back to
//! `XenonApp` on selection or dismissal.
//!
//! Matching runs off the UI thread (debounced) so large indexes cannot stall
//! typing. Prior results stay visible until the new match lands. Ranking lives
//! in `xenon_finder`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Task, Window,
};
use theme::ActiveTheme;
use xenon_design_system::{PaletteOverlay, palette_overlay};
use xenon_finder::{FileIndex, FileMatch};

use crate::palette::{
    PaletteLayout, ScrollResults, hint_row, reveal_selected, scroll_results, simple_row,
    step_selection,
};

/// Debounce before scoring a large index (keeps keystrokes snappy).
const QUERY_DEBOUNCE: Duration = Duration::from_millis(40);

pub enum FinderEvent {
    Selected(PathBuf),
    /// ⌘↩ / ⌃↩ / ⌘-click: open in a new pane to the right.
    SelectedBeside(PathBuf),
    RevealDir(PathBuf),
    Dismissed,
}

pub struct FinderView {
    /// `None` until the workspace index for this root has finished building.
    index: Option<Arc<FileIndex>>,
    /// Workspace-relative paths, most-recently opened first.
    recents: Vec<PathBuf>,
    query: String,
    /// Query string that `results` was computed for (stale guard for Enter).
    results_for: String,
    results: Vec<FileMatch>,
    selected: usize,
    focus: FocusHandle,
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    scroll: ScrollHandle,
    query_gen: u64,
    searching: bool,
    _query_task: Option<Task<()>>,
}

impl EventEmitter<FinderEvent> for FinderView {}

impl FinderView {
    pub fn new(
        index: Option<Arc<FileIndex>>,
        initial_query: String,
        recents: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            xenon_design_system::TextInputView::new(
                xenon_design_system::TextInputConfig::single_line(if index.is_some() {
                    "Search files…"
                } else {
                    "Indexing…"
                })
                .parent_navigation()
                .appearance(xenon_design_system::TextInputAppearance::Palette),
                cx,
            )
        });
        input.update(cx, |input, cx| {
            input.set_text(initial_query.clone(), cx);
            input.open(cx);
        });
        let focus = input.read(cx).focus_handle();
        let input_sub = cx.subscribe(&input, |this, _, event, cx| {
            if let xenon_design_system::TextInputEvent::Changed(query) = event {
                this.set_query(query.clone(), cx);
            }
        });
        let mut view = Self {
            index,
            recents,
            query: initial_query,
            results_for: String::new(),
            results: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
            scroll: ScrollHandle::new(),
            query_gen: 0,
            searching: false,
            _query_task: None,
        };
        view.kick_query(cx);
        view
    }

    pub fn set_index(&mut self, index: Arc<FileIndex>, cx: &mut Context<Self>) {
        self.index = Some(index);
        self.input
            .update(cx, |input, cx| input.set_placeholder("Search files…", cx));
        self.kick_query(cx);
        cx.notify();
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.selected = 0;
        self.kick_query(cx);
        cx.notify();
    }

    /// Debounce, then score on a background thread; apply only if still current.
    fn kick_query(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.index.clone() else {
            self.results.clear();
            self.results_for.clear();
            self.searching = false;
            self._query_task = None;
            return;
        };
        self.query_gen = self.query_gen.wrapping_add(1);
        let token = self.query_gen;
        let query = self.query.clone();
        let recents = self.recents.clone();
        self.searching = true;

        self._query_task = Some(cx.spawn(async move |this, cx| {
            // Empty query is cheap (slice); skip debounce so open feels instant.
            if !query.is_empty() {
                cx.background_executor().timer(QUERY_DEBOUNCE).await;
            }
            let still = this
                .update(cx, |this, _| this.query_gen == token && this.query == query)
                .unwrap_or(false);
            if !still {
                return;
            }
            let q = query.clone();
            let results = cx
                .background_executor()
                .spawn(async move { index.query(&q, &recents) })
                .await;
            this.update(cx, |this, cx| {
                if this.query_gen != token || this.query != query {
                    return;
                }
                this.results = results;
                this.results_for = query;
                this.searching = false;
                this.selected = 0;
                reveal_selected(&this.scroll, 0);
                cx.notify();
            })
            .ok();
        }));
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        step_selection(&mut self.selected, self.results.len(), delta, &self.scroll);
        cx.notify();
    }

    fn results_ready(&self) -> bool {
        self.results_for == self.query
    }

    fn confirm(&mut self, beside: bool, cx: &mut Context<Self>) {
        // Ignore Enter while results belong to a previous query.
        if !self.results_ready() {
            return;
        }
        if let Some(result) = self.results.get(self.selected) {
            if result.is_dir {
                cx.emit(FinderEvent::RevealDir(result.path.clone()));
            } else if beside {
                cx.emit(FinderEvent::SelectedBeside(result.path.clone()));
            } else {
                cx.emit(FinderEvent::Selected(result.path.clone()));
            }
        }
    }

    fn click_result(&mut self, index: usize, beside: bool, cx: &mut Context<Self>) {
        if !self.results_ready() {
            return;
        }
        self.selected = index;
        self.confirm(beside, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(FinderEvent::Dismissed),
            "enter" => {
                let beside =
                    event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                self.confirm(beside, cx);
            }
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn empty_label(&self) -> &'static str {
        if self.index.is_none() {
            "Indexing workspace…"
        } else if self.results.is_empty() && self.searching {
            "Searching…"
        } else if self.query.is_empty() && self.results.is_empty() {
            "Type to search files"
        } else if self.results.is_empty() {
            "No matching files"
        } else {
            // Non-empty list; label unused by scroll_results when rows exist.
            ""
        }
    }
}

impl Focusable for FinderView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FinderView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let empty = self.empty_label();
        // Keep prior rows visible while a newer query is in flight (large
        // indexes take tens of ms; clearing looks like "list emptied").
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let label = if m.is_dir {
                    format!("{}/", m.path.to_string_lossy())
                } else {
                    m.path.to_string_lossy().into_owned()
                };
                simple_row(("finder-row", i), label, i == self.selected, &colors)
                    .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                        let beside = event.modifiers().platform || event.modifiers().control;
                        this.click_result(i, beside, cx);
                    }))
                    .into_any_element()
            })
            .collect();

        palette_overlay(
            PaletteOverlay {
                id: "finder-scrim",
                layout,
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "Finder",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(FinderEvent::Dismissed),
                children: vec![
                    self.input.clone().into_any_element(),
                    scroll_results(ScrollResults {
                        list_id: "finder-results",
                        empty_message: empty,
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }),
                    hint_row("↩ open  ·  ⌘↩ beside", &colors).into_any_element(),
                ],
            },
            cx,
        )
    }
}
