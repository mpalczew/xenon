//! Run Task palette: fuzzy-pick a VS Code shell task. Enter starts a new
//! terminal tab and injects; cmd-enter injects into the current terminal.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xero_core::ShellTask;

use crate::impl_palette_query_input;
use crate::palette::{
    PaletteLayout, ScrollResults, fuzzy_index_order, hint_row, input_registrar, panel, query_row,
    reveal_selected, scrim, scroll_results, simple_row, step_selection,
};

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
    scroll: ScrollHandle,
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
            scroll: ScrollHandle::new(),
        };
        view.refilter();
        view
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self.tasks.iter().map(|t| t.label.clone()).collect();
        self.results = fuzzy_index_order(&haystacks, &self.query, &mut self.matcher);
        self.selected = 0;
        reveal_selected(&self.scroll, 0);
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        step_selection(&mut self.selected, self.results.len(), delta, &self.scroll);
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

    fn placeholder(&self) -> String {
        if let Some(msg) = &self.empty_message {
            msg.clone()
        } else {
            "Run task…".into()
        }
    }

    fn empty_message(&self) -> &'static str {
        if self.empty_message.is_some() {
            "No tasks loaded"
        } else if self.query.is_empty() {
            "No tasks in this workspace"
        } else {
            "No matching tasks"
        }
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
        let layout = PaletteLayout::default();
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, &task_i)| {
                let task = &self.tasks[task_i];
                let label = if let Some(detail) = &task.detail {
                    format!("{}  —  {}", task.label, detail)
                } else {
                    task.label.clone()
                };
                simple_row(("task-row", i), label, i == self.selected, &colors)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.confirm(true, cx);
                    }))
                    .into_any_element()
            })
            .collect();

        scrim("task-picker-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(TaskPickerEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("TaskPicker")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(
                        query_row(&self.query, &self.placeholder(), true, &colors)
                            .into_any_element(),
                    )
                    .child(scroll_results(ScrollResults {
                        list_id: "task-picker-results",
                        empty_message: self.empty_message(),
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }))
                    .child(
                        hint_row(
                            "↵ new terminal  ·  ⌘↵ current  ·  esc dismiss  ·  open ⌘⇧R",
                            &colors,
                        )
                        .into_any_element(),
                    ),
            )
    }
}

impl_palette_query_input!(TaskPickerView);
