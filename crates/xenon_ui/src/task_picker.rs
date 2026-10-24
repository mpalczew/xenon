//! Run Task palette: fuzzy-pick a VS Code task. Return runs it as its
//! presentation says; cmd-return runs it in a new tab of its own. Every row
//! shows the real command line, so a label cannot hide what runs.

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_design_system::{
    PaletteInput, PaletteOverlay, QueryRow, palette_input, palette_overlay, query_hint, query_row,
};

use crate::palette::{
    PaletteLayout, ScrollResults, fuzzy_index_order, match_hits, reveal_selected, scroll_results,
    step_selection,
};

/// One picker row, prepared by the model (`app/tasks.rs`).
#[derive(Clone, Debug)]
pub struct TaskRow {
    pub label: String,
    /// The command line as it will run (variables expanded where known).
    pub command_line: String,
    /// The task's own `detail` text.
    pub detail: Option<String>,
    /// Chip text, e.g. "build · default".
    pub tag: Option<String>,
    /// Why it cannot run here; such rows are listed dimmed.
    pub unsupported: Option<String>,
}

impl TaskRow {
    fn subtitle(&self) -> String {
        if let Some(reason) = &self.unsupported {
            return reason.clone();
        }
        match &self.detail {
            Some(detail) => format!("$ {}  ·  {detail}", self.command_line),
            None => format!("$ {}", self.command_line),
        }
    }
}

pub enum TaskPickerEvent {
    /// Run per the task's presentation settings.
    Run(String),
    /// Run in a new tab of its own, revealed.
    RunInNewTab(String),
    /// The row cannot run; the string says why.
    Unsupported(String),
    Dismissed,
}

pub struct TaskPickerView {
    tasks: Vec<TaskRow>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    focus: FocusHandle,
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    empty_message: Option<String>,
    scroll: ScrollHandle,
}

impl EventEmitter<TaskPickerEvent> for TaskPickerView {}

impl TaskPickerView {
    pub fn new(tasks: Vec<TaskRow>, error: Option<String>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            xenon_design_system::TextInputView::new(
                xenon_design_system::TextInputConfig::single_line(
                    error.as_deref().unwrap_or("Run task…"),
                )
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
            tasks,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
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

    fn confirm(&mut self, new_tab: bool, cx: &mut Context<Self>) {
        let Some(task) = self
            .results
            .get(self.selected)
            .and_then(|&idx| self.tasks.get(idx))
        else {
            return;
        };
        if let Some(reason) = &task.unsupported {
            cx.emit(TaskPickerEvent::Unsupported(format!(
                "{}: {reason}",
                task.label
            )));
        } else if new_tab {
            cx.emit(TaskPickerEvent::RunInNewTab(task.label.clone()));
        } else {
            cx.emit(TaskPickerEvent::Run(task.label.clone()));
        }
    }

    fn on_nav(&mut self, key: &str, _shift: bool, platform: bool, cx: &mut Context<Self>) {
        match key {
            "escape" => cx.emit(TaskPickerEvent::Dismissed),
            "enter" => self.confirm(platform, cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.on_nav(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.shift,
            event.keystroke.modifiers.secondary(),
            cx,
        );
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let mut ranker = Matcher::new(Config::DEFAULT);
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, &task_i)| {
                let task = &self.tasks[task_i];
                let hits = match_hits(&task.label, &self.query, &mut ranker);
                query_row(
                    ("task-row", i),
                    QueryRow {
                        title: task.label.clone(),
                        detail: task
                            .unsupported
                            .as_ref()
                            .map(|_| "unsupported".to_string())
                            .or_else(|| task.tag.clone()),
                        subtitle: Some(task.subtitle()),
                        selected: i == self.selected,
                        enabled: task.unsupported.is_none(),
                        hits,
                    },
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    this.confirm(false, cx);
                }))
                .into_any_element()
            })
            .collect();

        palette_overlay(
            PaletteOverlay {
                id: "task-picker-scrim",
                layout,
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "TaskPicker",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(TaskPickerEvent::Dismissed),
                children: vec![
                    self.input.clone().into_any_element(),
                    scroll_results(ScrollResults {
                        list_id: "task-picker-results",
                        empty_message: self.empty_message(),
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }),
                    query_hint(
                        &xenon_design_system::shortcut_text(
                            "return runs  ·  ⌘return runs in a new tab  ·  esc closes",
                        ),
                        cx,
                    )
                    .into_any_element(),
                ],
            },
            cx,
        )
    }
}
