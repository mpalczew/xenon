//! `FinderView`: the cmd-p fuzzy file palette. Emits `FinderEvent` back to
//! `XeroApp` on selection or dismissal. Text input uses the same
//! `EntityInputHandler` + registrar-canvas pattern as the editor/terminal.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Point, Render, StatefulInteractiveElement, Styled, UTF16Selection, Window, canvas, div, px,
};
use theme::ActiveTheme;
use xero_finder::{FileIndex, FileMatch, Finder};

/// How many ranked results to show at once.
const VISIBLE_RESULTS: usize = 20;

pub enum FinderEvent {
    Selected(PathBuf),
    RevealDir(PathBuf),
    Dismissed,
}

pub struct FinderView {
    /// `None` until the workspace index for this root has finished building in
    /// the background; the query row shows "Indexing…" until then.
    finder: Option<Finder>,
    query: String,
    results: Vec<FileMatch>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
}

impl EventEmitter<FinderEvent> for FinderView {}

impl FinderView {
    /// Open over `index` (or `None` if it is still building) with an optional
    /// prefilled `initial_query` (used when a cmd-clicked name is ambiguous).
    pub fn new(
        index: Option<Arc<FileIndex>>,
        initial_query: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut finder = index.map(Finder::new);
        let results = finder
            .as_mut()
            .map(|f| f.query(&initial_query))
            .unwrap_or_default();
        Self {
            finder,
            query: initial_query,
            results,
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
        }
    }

    /// Swap in the index once its background build completes, re-running the
    /// current query so results appear without the user retyping.
    pub fn set_index(&mut self, index: Arc<FileIndex>, cx: &mut Context<Self>) {
        let mut finder = Finder::new(index);
        self.results = finder.query(&self.query);
        self.finder = Some(finder);
        self.selected = 0;
        cx.notify();
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.results = self
            .finder
            .as_mut()
            .map(|f| f.query(&self.query))
            .unwrap_or_default();
        self.selected = 0;
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() - 1;
        let next = (self.selected as isize + delta).clamp(0, last as isize);
        self.selected = next as usize;
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(result) = self.results.get(self.selected) {
            if result.is_dir {
                cx.emit(FinderEvent::RevealDir(result.path.clone()));
            } else {
                cx.emit(FinderEvent::Selected(result.path.clone()));
            }
        }
    }

    /// Clicking a result selects and confirms it in one gesture.
    fn click_result(&mut self, index: usize, cx: &mut Context<Self>) {
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
        }
        let colors = cx.theme().colors().clone();
        // A full-window scrim that centers the palette near the top. Clicking the
        // scrim (outside the panel) dismisses; the panel occludes clicks so they
        // don't reach the scrim.
        div()
            .id("finder-scrim")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(FinderEvent::Dismissed)))
            .child(
                div()
                    .occlude()
                    .track_focus(&self.focus)
                    .key_context("Finder")
                    .on_key_down(cx.listener(Self::on_key))
                    .relative()
                    .w(px(640.))
                    .max_h(px(420.))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.elevated_surface_background)
                    // The input registrar is a transparent full-size canvas; keep
                    // it first so it paints underneath and never intercepts clicks
                    // meant for the result rows.
                    .child(input_registrar(cx.entity(), self.focus.clone()))
                    .child(self.query_row(cx))
                    .child(self.results_list(cx)),
            )
    }
}

impl FinderView {
    fn query_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let shown = if !self.query.is_empty() {
            self.query.clone()
        } else if self.finder.is_none() {
            "Indexing…".to_string()
        } else {
            "Search files…".to_string()
        };
        div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .text_color(if self.query.is_empty() {
                colors.text_muted
            } else {
                colors.text
            })
            .child(shown)
    }

    fn results_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let rows: Vec<_> = self
            .results
            .iter()
            .take(VISIBLE_RESULTS)
            .enumerate()
            .map(|(i, m)| {
                let label = if m.is_dir {
                    format!("{}/", m.path.to_string_lossy())
                } else {
                    m.path.to_string_lossy().into_owned()
                };
                let mut row = div()
                    .id(("finder-row", i))
                    .px_3()
                    .py_1()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.click_result(i, cx)));
                if i == self.selected {
                    row = row.bg(colors.element_selected);
                }
                row
            })
            .collect();
        div().flex().flex_col().overflow_hidden().children(rows)
    }
}

/// A transparent full-size canvas whose only job is to register the text input
/// handler during paint (required to receive typed characters on macOS).
fn input_registrar(view: Entity<FinderView>, focus: FocusHandle) -> impl IntoElement {
    canvas(
        move |_bounds, _window, _cx| {},
        move |bounds, _prepaint, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

impl EntityInputHandler for FinderView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !text.is_empty() {
            let query = format!("{}{}", self.query, text);
            self.set_query(query, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !new_text.is_empty() {
            let query = format!("{}{}", self.query, new_text);
            self.set_query(query, cx);
        }
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
