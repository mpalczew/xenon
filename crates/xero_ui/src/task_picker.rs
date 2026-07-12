//! Run Task palette: fuzzy-pick a VS Code shell task. Enter starts a new
//! terminal tab and injects; cmd-enter injects into the current terminal.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    Point, Render, StatefulInteractiveElement, Styled, UTF16Selection, Window, canvas, div, px,
};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xero_core::ShellTask;

const VISIBLE: usize = 20;

pub enum TaskPickerEvent {
    /// Run in the current terminal.
    Run(ShellTask),
    /// Open a new terminal tab, then run.
    RunInNew(ShellTask),
    Dismissed,
}

pub struct TaskPickerView {
    tasks: Vec<ShellTask>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
    matcher: Matcher,
    empty_message: Option<String>,
}

impl EventEmitter<TaskPickerEvent> for TaskPickerView {}

impl TaskPickerView {
    pub fn new(tasks: Vec<ShellTask>, error: Option<String>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            tasks,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
            matcher: Matcher::new(Config::DEFAULT),
            empty_message: error,
        };
        view.refilter();
        view
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.results = (0..self.tasks.len()).collect();
        } else {
            let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
            let labels: Vec<&str> = self.tasks.iter().map(|t| t.label.as_str()).collect();
            let mut scored: Vec<(usize, u32)> = pattern
                .match_list(labels.iter().copied(), &mut self.matcher)
                .into_iter()
                .filter_map(|(label, score)| {
                    self.tasks
                        .iter()
                        .position(|t| t.label == label)
                        .map(|i| (i, score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            self.results = scored.into_iter().map(|(i, _)| i).collect();
        }
        self.selected = 0;
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
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

    fn confirm(&mut self, new_terminal: bool, cx: &mut Context<Self>) {
        let Some(&idx) = self.results.get(self.selected) else {
            return;
        };
        let Some(task) = self.tasks.get(idx).cloned() else {
            return;
        };
        if new_terminal {
            cx.emit(TaskPickerEvent::RunInNew(task));
        } else {
            cx.emit(TaskPickerEvent::Run(task));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(TaskPickerEvent::Dismissed),
            "enter" => {
                // Default: new terminal. ⌘Enter: current terminal.
                let current = event.keystroke.modifiers.platform;
                self.confirm(!current, cx);
            }
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

impl Focusable for TaskPickerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TaskPickerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        div()
            .id("task-picker-scrim")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(TaskPickerEvent::Dismissed)))
            .child(
                div()
                    .occlude()
                    .track_focus(&self.focus)
                    .key_context("TaskPicker")
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
                    .child(input_registrar(cx.entity(), self.focus.clone()))
                    .child(self.query_row(cx))
                    .child(self.results_list(cx))
                    .child(self.hint_row(cx)),
            )
    }
}

impl TaskPickerView {
    fn query_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let shown = if !self.query.is_empty() {
            self.query.clone()
        } else if let Some(msg) = &self.empty_message {
            msg.clone()
        } else {
            "Run task…".to_string()
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

    fn hint_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .px_3()
            .py_1()
            .border_t_1()
            .border_color(colors.border)
            .text_xs()
            .text_color(colors.text_muted)
            .child("↵ new terminal  ·  ⌘↵ current  ·  esc dismiss  ·  open ⌘⇧R")
    }

    fn results_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let rows: Vec<_> = self
            .results
            .iter()
            .take(VISIBLE)
            .enumerate()
            .map(|(i, &task_i)| {
                let task = &self.tasks[task_i];
                let label = if let Some(detail) = &task.detail {
                    format!("{}  —  {}", task.label, detail)
                } else {
                    task.label.clone()
                };
                let mut row = div()
                    .id(("task-row", i))
                    .px_3()
                    .py_1()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.confirm(true, cx);
                    }));
                if i == self.selected {
                    row = row.bg(colors.element_selected);
                }
                row
            })
            .collect();
        div().flex().flex_col().overflow_hidden().children(rows)
    }
}

fn input_registrar(view: Entity<TaskPickerView>, focus: FocusHandle) -> impl IntoElement {
    canvas(
        move |_bounds, _window, _cx| {},
        move |bounds, _prepaint, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

impl EntityInputHandler for TaskPickerView {
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
