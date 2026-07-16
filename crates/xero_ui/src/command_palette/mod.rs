//! Command palette (⌘⇧P), stream jump (⌘T), and keyboard help (⌘⇧/).
//! Shell chrome lives in `crate::palette`; this file owns catalog + routing.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Window,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xero_core::{StreamId, WorkspaceId};

use crate::commands::{CommandEntry, CommandId, catalog};
use crate::impl_palette_query_input;
use crate::palette::{
    DetailRow, PaletteLayout, clamp_selection, detail_row, fuzzy_index_order, hint_row,
    input_registrar, optional_title, panel, query_row, scrim, scroll_results,
};

#[derive(Clone, Debug)]
pub enum PaletteItem {
    Command(CommandEntry),
    Stream { id: StreamId, label: String },
    Workspace { id: WorkspaceId, label: String },
}

impl PaletteItem {
    fn haystack(&self) -> String {
        match self {
            Self::Command(e) => format!("{} {} {}", e.label, e.keys, e.group),
            Self::Stream { label, .. } | Self::Workspace { label, .. } => label.clone(),
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Command(e) => e.label.to_string(),
            Self::Stream { label, .. } | Self::Workspace { label, .. } => label.clone(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Command(e) => e.keys.to_string(),
            Self::Stream { .. } => "stream".into(),
            Self::Workspace { .. } => "workspace".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Streams,
    Help,
}

pub enum CommandPaletteEvent {
    Run(CommandId),
    ActivateStream(StreamId),
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
    focused_once: bool,
    matcher: Matcher,
}

impl EventEmitter<CommandPaletteEvent> for CommandPaletteView {}

impl CommandPaletteView {
    pub fn new(
        mode: PaletteMode,
        streams: Vec<(StreamId, String)>,
        workspaces: Vec<(WorkspaceId, String)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut items = Vec::new();
        match mode {
            PaletteMode::Streams => {
                for (id, label) in streams {
                    items.push(PaletteItem::Stream { id, label });
                }
            }
            PaletteMode::Commands | PaletteMode::Help => {
                for entry in catalog() {
                    items.push(PaletteItem::Command(*entry));
                }
                if mode == PaletteMode::Commands {
                    for (id, label) in streams {
                        items.push(PaletteItem::Stream { id, label });
                    }
                    for (id, label) in workspaces {
                        items.push(PaletteItem::Workspace { id, label });
                    }
                }
            }
        }
        let mut view = Self {
            mode,
            items,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
            matcher: Matcher::new(Config::DEFAULT),
        };
        view.refilter();
        view
    }

    fn placeholder(&self) -> &'static str {
        match self.mode {
            PaletteMode::Commands => "Run command or jump…",
            PaletteMode::Streams => "Go to stream…",
            PaletteMode::Help => "Filter shortcuts…",
        }
    }

    fn title(&self) -> &'static str {
        match self.mode {
            PaletteMode::Commands => "Commands",
            PaletteMode::Streams => "Streams",
            PaletteMode::Help => "Keyboard Shortcuts",
        }
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self.items.iter().map(|i| i.haystack()).collect();
        self.results = fuzzy_index_order(&haystacks, &self.query, &mut self.matcher);
        self.selected = 0;
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.selected = clamp_selection(self.selected, self.results.len(), delta);
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
            PaletteItem::Stream { id, .. } => cx.emit(CommandPaletteEvent::ActivateStream(id)),
            PaletteItem::Workspace { id, .. } => {
                cx.emit(CommandPaletteEvent::ActivateWorkspace(id))
            }
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(CommandPaletteEvent::Dismissed),
            "enter" => self.confirm(cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "backspace" => {
                let mut q = self.query.clone();
                q.pop();
                self.set_query(q, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Focusable for CommandPaletteView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::tall();
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, &item_i)| {
                let item = &self.items[item_i];
                detail_row(
                    ("cmd-row", i),
                    DetailRow {
                        title: item.title(),
                        detail: item.detail(),
                        selected: i == self.selected,
                        selectable: true,
                        subtitle: None,
                    },
                    &colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    this.confirm(cx);
                }))
                .into_any_element()
            })
            .collect();

        scrim("command-palette-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(CommandPaletteEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("CommandPalette")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(optional_title(self.title(), &colors).into_any_element())
                    .child(query_row(&self.query, self.placeholder(), &colors).into_any_element())
                    .child(scroll_results(
                        "command-palette-results",
                        "No matches",
                        rows,
                        &colors,
                    ))
                    .child(
                        hint_row("↵ run  ·  esc dismiss  ·  type to filter", &colors)
                            .into_any_element(),
                    ),
            )
    }
}

impl_palette_query_input!(CommandPaletteView);
