//! Command palette (⌘⇧P) and keyboard help (⌘⇧/).
//! Shell chrome lives in `crate::palette`; this file owns catalog + routing.

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Render, ScrollHandle, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_core::WorkspaceId;
use xenon_design_system::{
    PaletteInput, PaletteOverlay, QueryRow, palette_input, palette_overlay, query_hint,
    query_label, query_row,
};

use crate::commands::{CommandEntry, CommandId, catalog};
use crate::palette::{
    PaletteLayout, ScrollResults, fuzzy_index_order, match_hits, reveal_selected, scroll_results,
    step_selection,
};

#[derive(Clone, Debug)]
pub enum PaletteItem {
    Command(CommandEntry),
    Workspace { id: WorkspaceId, label: String },
}

impl PaletteItem {
    fn haystack(&self) -> String {
        match self {
            Self::Command(e) => format!(
                "{} {} {}",
                e.label,
                xenon_keymap::display_keys(e.keys),
                e.group
            ),
            Self::Workspace { label, .. } => label.clone(),
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Command(e) => e.label.to_string(),
            Self::Workspace { label, .. } => label.clone(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Command(e) => xenon_keymap::display_keys(e.keys),
            Self::Workspace { .. } => "workspace".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Help,
}

pub enum CommandPaletteEvent {
    Run(CommandId),
    ActivateWorkspace(WorkspaceId),
    Dismissed,
}

pub struct CommandPaletteView {
    mode: PaletteMode,
    items: Vec<PaletteItem>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    focus: FocusHandle,
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    scroll: ScrollHandle,
}

impl EventEmitter<CommandPaletteEvent> for CommandPaletteView {}

impl CommandPaletteView {
    pub fn new(
        mode: PaletteMode,
        workspaces: Vec<(WorkspaceId, String)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut items = Vec::new();
        match mode {
            PaletteMode::Commands | PaletteMode::Help => {
                for entry in catalog() {
                    items.push(PaletteItem::Command(*entry));
                }
                if mode == PaletteMode::Commands {
                    for (id, label) in workspaces {
                        items.push(PaletteItem::Workspace { id, label });
                    }
                }
            }
        }
        let placeholder = if mode == PaletteMode::Commands {
            "Run command or jump…"
        } else {
            "Filter shortcuts…"
        };
        let input = cx.new(|cx| {
            xenon_design_system::TextInputView::new(
                xenon_design_system::TextInputConfig::single_line(placeholder)
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
            mode,
            items,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
        };
        view.refilter();
        view
    }

    fn title(&self) -> &'static str {
        match self.mode {
            PaletteMode::Commands => "Commands",
            PaletteMode::Help => "Keyboard Shortcuts",
        }
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self.items.iter().map(|i| i.haystack()).collect();
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

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(&idx) = self.results.get(self.selected) else {
            return;
        };
        let Some(item) = self.items.get(idx).cloned() else {
            return;
        };
        match item {
            PaletteItem::Command(entry) => cx.emit(CommandPaletteEvent::Run(entry.id)),
            PaletteItem::Workspace { id, .. } => {
                cx.emit(CommandPaletteEvent::ActivateWorkspace(id))
            }
        }
    }

    fn on_nav(&mut self, key: &str, _shift: bool, _platform: bool, cx: &mut Context<Self>) {
        match key {
            "escape" => cx.emit(CommandPaletteEvent::Dismissed),
            "enter" => self.confirm(cx),
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
}

impl Focusable for CommandPaletteView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::tall();
        let mut ranker = Matcher::new(Config::DEFAULT);
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, &item_i)| {
                let item = &self.items[item_i];
                let title = item.title();
                let hits = match_hits(&title, &self.query, &mut ranker);
                query_row(
                    ("cmd-row", i),
                    QueryRow {
                        title,
                        detail: Some(item.detail()),
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

        palette_overlay(
            PaletteOverlay {
                id: "command-palette-scrim",
                layout,
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "CommandPalette",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(CommandPaletteEvent::Dismissed),
                children: vec![
                    query_label(self.title(), cx).into_any_element(),
                    self.input.clone().into_any_element(),
                    scroll_results(ScrollResults {
                        list_id: "command-palette-results",
                        empty_message: "No matches",
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }),
                    query_hint("return runs  ·  esc closes", cx).into_any_element(),
                ],
            },
            cx,
        )
    }
}
