//! `FinderView`: the cmd-p fuzzy file palette. Emits `FinderEvent` back to
//! `XeroApp` on selection or dismissal. Text input uses the same
//! `EntityInputHandler` + registrar-canvas pattern as the editor/terminal.

use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Point, Render, Styled, UTF16Selection, Window, canvas, div, px,
};
use theme::ActiveTheme;
use xero_finder::{FileMatch, Finder};

/// How many ranked results to show at once.
const VISIBLE_RESULTS: usize = 20;

pub enum FinderEvent {
    Selected(PathBuf),
    Dismissed,
}

pub struct FinderView {
    finder: Finder,
    query: String,
    results: Vec<FileMatch>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
}

impl EventEmitter<FinderEvent> for FinderView {}

impl FinderView {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut finder = Finder::start(&root);
        let results = finder.query("");
        Self {
            finder,
            query: String::new(),
            results,
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
        }
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.results = self.finder.query(&self.query);
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
            cx.emit(FinderEvent::Selected(result.path.clone()));
        }
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
        // A full-window scrim that centers the palette near the top.
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.))
            .child(
                div()
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
                    .child(self.query_row(cx))
                    .child(self.results_list(cx))
                    .child(input_registrar(cx.entity(), self.focus.clone())),
            )
    }
}

impl FinderView {
    fn query_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let shown = if self.query.is_empty() {
            "Search files…".to_string()
        } else {
            self.query.clone()
        };
        div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .text_color(if self.query.is_empty() { colors.text_muted } else { colors.text })
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
                let mut row = div()
                    .px_3()
                    .py_1()
                    .text_sm()
                    .child(m.path.to_string_lossy().into_owned());
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
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
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
