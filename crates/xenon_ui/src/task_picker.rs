//! Run Task palette: fuzzy-pick a VS Code shell task. Enter starts a new
//! terminal tab and injects; cmd-enter injects into the current terminal.

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_core::ShellTask;
use xenon_design_system::{
    PaletteInput, PaletteOverlay, QueryRow, palette_input, palette_overlay, query_hint, query_row,
};

use crate::palette::{
    PaletteLayout, ScrollResults, fuzzy_index_order, match_hits, reveal_selected, scroll_results,
    step_selection,
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
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    empty_message: Option<String>,
    scroll: ScrollHandle,
}

impl EventEmitter<TaskPickerEvent> for TaskPickerView {}

impl TaskPickerView {
    pub fn new(tasks: Vec<ShellTask>, error: Option<String>, cx: &mut Context<Self>) -> Self {
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

    fn on_nav(&mut self, key: &str, _shift: bool, platform: bool, cx: &mut Context<Self>) {
        match key {
            "escape" => cx.emit(TaskPickerEvent::Dismissed),
            "enter" => self.confirm(!platform, cx),
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
                        detail: task.detail.clone(),
                        subtitle: None,
                        selected: i == self.selected,
                        enabled: true,
                        hits,
                    },
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    this.confirm(true, cx);
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
                            "return runs in a new terminal  ·  ⌘return current  ·  esc closes",
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
