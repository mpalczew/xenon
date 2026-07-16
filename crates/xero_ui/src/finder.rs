//! `FinderView`: the cmd-p fuzzy file palette. Emits `FinderEvent` back to
//! `XeroApp` on selection or dismissal.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Window,
};
use theme::ActiveTheme;
use xero_finder::{FileIndex, FileMatch, Finder};

use crate::impl_palette_query_input;
use crate::palette::{
    PaletteLayout, clamp_selection, input_registrar, panel, query_row, scrim, scroll_results,
    simple_row,
};

pub enum FinderEvent {
    Selected(PathBuf),
    RevealDir(PathBuf),
    Dismissed,
}

pub struct FinderView {
    /// `None` until the workspace index for this root has finished building.
    finder: Option<Finder>,
    query: String,
    results: Vec<FileMatch>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
}

impl EventEmitter<FinderEvent> for FinderView {}

impl FinderView {
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
        self.selected = clamp_selection(self.selected, self.results.len(), delta);
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

    fn placeholder(&self) -> &'static str {
        if self.finder.is_none() {
            "Indexing…"
        } else {
            "Search files…"
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
        }
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let empty = if self.finder.is_none() {
            "Indexing workspace…"
        } else if self.query.is_empty() {
            "Type to search files"
        } else {
            "No matching files"
        };
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
                    .child(query_row(&self.query, self.placeholder(), &colors).into_any_element())
                    .child(scroll_results("finder-results", empty, rows, &colors)),
            )
    }
}

impl_palette_query_input!(FinderView);
