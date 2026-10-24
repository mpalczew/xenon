//! Prompt for a tasks.json `inputs` entry: `promptString` (optionally a
//! password, shown as bullets and never copied) or `pickString`. Return
//! answers, Escape cancels the whole run.

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_core::PickOption;
use xenon_design_system::{
    PaletteInput, PaletteOverlay, QueryRow, palette_input, palette_overlay, query_hint,
    query_label, query_row,
};

use crate::palette::{
    PaletteLayout, ScrollResults, fuzzy_index_order, match_hits, reveal_selected, scroll_results,
    step_selection,
};

/// What to ask. The model builds it from an `InputDef`.
#[derive(Clone, Debug)]
pub struct TaskQuestion {
    pub id: String,
    pub description: String,
    pub default: Option<String>,
    pub kind: QuestionKind,
}

#[derive(Clone, Debug)]
pub enum QuestionKind {
    Text { password: bool },
    Pick { options: Vec<PickOption> },
}

pub enum TaskInputEvent {
    Answered(String),
    Cancelled,
}

pub struct TaskInputView {
    question: TaskQuestion,
    query: String,
    results: Vec<usize>,
    selected: usize,
    focus: FocusHandle,
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    scroll: ScrollHandle,
}

impl EventEmitter<TaskInputEvent> for TaskInputView {}

impl TaskInputView {
    pub fn new(question: TaskQuestion, cx: &mut Context<Self>) -> Self {
        let (placeholder, password) = match &question.kind {
            QuestionKind::Text { password } => ("Value…", *password),
            QuestionKind::Pick { .. } => ("Filter…", false),
        };
        let input = cx.new(|cx| {
            let mut config = xenon_design_system::TextInputConfig::single_line(placeholder)
                .parent_navigation()
                .appearance(xenon_design_system::TextInputAppearance::Palette);
            if password {
                config.masked = true;
            }
            xenon_design_system::TextInputView::new(config, cx)
        });
        let is_text = matches!(question.kind, QuestionKind::Text { .. });
        let query = if is_text {
            question.default.clone().unwrap_or_default()
        } else {
            String::new()
        };
        input.update(cx, |input, cx| {
            input.set_text(query.clone(), cx);
            input.open(cx);
        });
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
            question,
            query,
            results: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
        };
        view.refilter();
        view.select_default();
        view
    }

    /// Visual scenes: type into the field as a user would.
    #[cfg(feature = "visual-tests")]
    pub fn visual_type(&mut self, text: &str, cx: &mut Context<Self>) {
        self.query = text.to_string();
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        cx.notify();
    }

    fn options(&self) -> &[PickOption] {
        match &self.question.kind {
            QuestionKind::Pick { options } => options,
            QuestionKind::Text { .. } => &[],
        }
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self.options().iter().map(|o| o.label.clone()).collect();
        let query = if self.is_pick() { &self.query } else { "" };
        self.results = fuzzy_index_order(&haystacks, query, &mut self.matcher);
        self.selected = 0;
        reveal_selected(&self.scroll, 0);
    }

    fn select_default(&mut self) {
        let Some(default) = self.question.default.as_deref() else {
            return;
        };
        let position = self.results.iter().position(|&i| {
            self.options()[i].value == default || self.options()[i].label == default
        });
        if let Some(position) = position {
            self.selected = position;
            reveal_selected(&self.scroll, position);
        }
    }

    fn is_pick(&self) -> bool {
        matches!(self.question.kind, QuestionKind::Pick { .. })
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.is_pick() {
            let value = self
                .results
                .get(self.selected)
                .map(|&i| self.options()[i].value.clone());
            if let Some(value) = value {
                cx.emit(TaskInputEvent::Answered(value));
            }
        } else {
            cx.emit(TaskInputEvent::Answered(self.query.clone()));
        }
    }

    fn on_nav(&mut self, key: &str, _shift: bool, _platform: bool, cx: &mut Context<Self>) {
        match key {
            "escape" => cx.emit(TaskInputEvent::Cancelled),
            "enter" => self.confirm(cx),
            "up" if self.is_pick() => {
                step_selection(&mut self.selected, self.results.len(), -1, &self.scroll)
            }
            "down" if self.is_pick() => {
                step_selection(&mut self.selected, self.results.len(), 1, &self.scroll)
            }
            _ => return,
        }
        cx.notify();
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

    fn prompt_text(&self) -> String {
        if self.question.description.is_empty() {
            self.question.id.clone()
        } else {
            self.question.description.clone()
        }
    }
}

impl Focusable for TaskInputView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TaskInputView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let mut children = vec![
            query_label(&self.prompt_text(), cx).into_any_element(),
            self.input.clone().into_any_element(),
        ];
        let hint = match &self.question.kind {
            QuestionKind::Text { password: true } => {
                "return confirms  ·  esc cancels  ·  hidden, never saved or echoed"
            }
            QuestionKind::Text { .. } => "return confirms  ·  esc cancels",
            QuestionKind::Pick { .. } => "↑↓ choose  ·  return confirms  ·  esc cancels",
        };
        if self.is_pick() {
            let mut ranker = Matcher::new(Config::DEFAULT);
            let rows: Vec<_> = self
                .results
                .iter()
                .enumerate()
                .map(|(i, &option_i)| {
                    let option = &self.options()[option_i];
                    let hits = match_hits(&option.label, &self.query, &mut ranker);
                    let detail = (option.value != option.label).then(|| option.value.clone());
                    query_row(
                        ("task-input-row", i),
                        QueryRow {
                            title: option.label.clone(),
                            detail,
                            subtitle: None,
                            selected: i == self.selected,
                            enabled: true,
                            hits,
                        },
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.confirm(cx);
                    }))
                    .into_any_element()
                })
                .collect();
            children.push(scroll_results(ScrollResults {
                list_id: "task-input-results",
                empty_message: "No matching choices",
                rows,
                selected: self.selected,
                scroll: &self.scroll,
                colors: &colors,
            }));
        }
        children.push(query_hint(hint, cx).into_any_element());
        palette_overlay(
            PaletteOverlay {
                id: "task-input-scrim",
                layout: PaletteLayout::default(),
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "TaskInput",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(TaskInputEvent::Cancelled),
                children,
            },
            cx,
        )
    }
}
