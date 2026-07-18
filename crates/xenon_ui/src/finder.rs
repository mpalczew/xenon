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
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Task, Window,
};
use theme::ActiveTheme;
use xenon_finder::{FileIndex, FileMatch};

use crate::impl_palette_query_input;
use crate::palette::{
    PaletteLayout, ScrollResults, input_registrar, panel, query_row, reveal_selected, scrim,
    scroll_results, simple_row, step_selection,
};

/// Debounce before scoring a large index (keeps keystrokes snappy).
const QUERY_DEBOUNCE: Duration = Duration::from_millis(40);
const CARET_BLINK: Duration = Duration::from_millis(530);

pub enum FinderEvent {
    Selected(PathBuf),
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
    focused_once: bool,
    scroll: ScrollHandle,
    query_gen: u64,
    searching: bool,
    caret_on: bool,
    _query_task: Option<Task<()>>,
    _blink: Option<Task<()>>,
}

impl EventEmitter<FinderEvent> for FinderView {}

impl FinderView {
    pub fn new(
        index: Option<Arc<FileIndex>>,
        initial_query: String,
        recents: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            index,
            recents,
            query: initial_query,
            results_for: String::new(),
            results: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
            scroll: ScrollHandle::new(),
            query_gen: 0,
            searching: false,
            caret_on: true,
            _query_task: None,
            _blink: None,
        };
        view.kick_query(cx);
        view
    }

    pub fn set_index(&mut self, index: Arc<FileIndex>, cx: &mut Context<Self>) {
        self.index = Some(index);
        self.kick_query(cx);
        cx.notify();
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.selected = 0;
        self.caret_on = true;
        self.kick_query(cx);
        cx.notify();
    }

    fn start_caret_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        self._blink = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CARET_BLINK).await;
                let keep = this
                    .update(cx, |this, cx| {
                        this.caret_on = !this.caret_on;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
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

    fn confirm(&mut self, cx: &mut Context<Self>) {
        // Ignore Enter while results belong to a previous query.
        if !self.results_ready() {
            return;
        }
        if let Some(result) = self.results.get(self.selected) {
            if result.is_dir {
                cx.emit(FinderEvent::RevealDir(result.path.clone()));
            } else {
                cx.emit(FinderEvent::Selected(result.path.clone()));
            }
        }
    }

    fn click_result(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.results_ready() {
            return;
        }
        self.selected = index;
        self.confirm(cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(FinderEvent::Dismissed),
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

    fn placeholder(&self) -> &'static str {
        if self.index.is_none() {
            "Indexing…"
        } else {
            "Search files…"
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
            self.start_caret_blink(cx);
        }
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
                    .on_click(cx.listener(move |this, _, _, cx| this.click_result(i, cx)))
                    .into_any_element()
            })
            .collect();

        scrim("finder-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(FinderEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("Finder")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(
                        query_row(&self.query, self.placeholder(), self.caret_on, &colors)
                            .into_any_element(),
                    )
                    .child(scroll_results(ScrollResults {
                        list_id: "finder-results",
                        empty_message: empty,
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    })),
            )
    }
}

impl_palette_query_input!(FinderView);
