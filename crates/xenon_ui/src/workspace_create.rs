//! Keyboard-first creator for a new directory-backed workspace.

use std::path::PathBuf;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window,
    div,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;

use crate::impl_palette_query_input;
use crate::palette::{
    DetailRow, PaletteLayout, ScrollResults, detail_row, fuzzy_index_order, hint_row,
    input_registrar, panel, query_row, reveal_selected, scrim, scroll_results,
};
use crate::workspace_discover::{discover_parent_dirs, resolve_existing_dir};

pub enum WorkspaceCreateEvent {
    Create { name: String, parent: PathBuf },
    Dismissed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Name,
    Parent,
}

pub struct WorkspaceCreateView {
    step: Step,
    name: String,
    query: String,
    parents: Vec<PathBuf>,
    results: Vec<PathBuf>,
    selected: usize,
    focus: FocusHandle,
    focused_once: bool,
    matcher: Matcher,
    scroll: ScrollHandle,
    _discover_task: Option<gpui::Task<()>>,
}

impl EventEmitter<WorkspaceCreateEvent> for WorkspaceCreateView {}

impl WorkspaceCreateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            step: Step::Name,
            name: String::new(),
            query: String::new(),
            parents: Vec::new(),
            results: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            focused_once: false,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
            _discover_task: None,
        };
        view.load_parent_dirs(cx);
        view
    }

    fn load_parent_dirs(&mut self, cx: &mut Context<Self>) {
        self._discover_task = Some(cx.spawn(async move |this, cx| {
            let dirs = cx
                .background_executor()
                .spawn(async { discover_parent_dirs() })
                .await;
            this.update(cx, |this, cx| {
                this.parents = dirs;
                this.refilter();
                cx.notify();
            })
            .ok();
        }));
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self
            .parents
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        self.results = if self.query.trim().is_empty() {
            self.parents.iter().take(12).cloned().collect()
        } else {
            fuzzy_index_order(&haystacks, &self.query, &mut self.matcher)
                .into_iter()
                .map(|i| self.parents[i].clone())
                .take(20)
                .collect()
        };
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
        cx.notify();
    }

    fn selected_parent(&self) -> Option<PathBuf> {
        self.results.get(self.selected).cloned()
    }

    fn valid_name(&self) -> bool {
        let name = self.query.trim();
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.contains('/')
            && !name.contains('\\')
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.step == Step::Name && self.valid_name() {
            self.name = self.query.trim().to_string();
            self.step = Step::Parent;
            self.query.clear();
            self.selected = 0;
            self.refilter();
            cx.notify();
        }
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Name => self.advance(cx),
            Step::Parent => {
                let Some(parent) = self.selected_parent() else {
                    return;
                };
                if resolve_existing_dir(&parent).is_some() && self.valid_name() {
                    cx.emit(WorkspaceCreateEvent::Create {
                        name: self.name.trim().to_string(),
                        parent,
                    });
                }
            }
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        self.selected =
            (self.selected as isize + delta).clamp(0, self.results.len() as isize - 1) as usize;
        reveal_selected(&self.scroll, self.selected);
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" if self.step == Step::Parent => {
                self.step = Step::Name;
                self.query.clear();
                cx.notify();
            }
            "escape" => cx.emit(WorkspaceCreateEvent::Dismissed),
            "enter" => self.confirm(cx),
            "tab" if self.step == Step::Name => self.advance(cx),
            "up" if self.step == Step::Parent => self.move_selection(-1, cx),
            "down" if self.step == Step::Parent => self.move_selection(1, cx),
            "backspace" => {
                let mut query = self.query.clone();
                query.pop();
                if self.step == Step::Name {
                    self.query = query;
                    cx.notify();
                } else {
                    self.set_query(query, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn render_parent_results(
        &self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let selected = i == self.selected;
                detail_row(
                    ("workspace-parent", i),
                    DetailRow {
                        title: path
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_default()
                            .to_string(),
                        detail: "folder".to_string(),
                        selected,
                        selectable: true,
                        subtitle: Some(display_path(path)),
                    },
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    this.confirm(cx);
                }))
                .into_any_element()
            })
            .collect();
        scroll_results(ScrollResults {
            list_id: "workspace-parent-results",
            empty_message: "No folders found",
            rows,
            selected: self.selected,
            scroll: &self.scroll,
            colors,
        })
    }
}

impl Focusable for WorkspaceCreateView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WorkspaceCreateView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let title = match self.step {
            Step::Name => "1  Name workspace directory",
            Step::Parent => "2  Choose parent folder",
        };
        let body = match self.step {
            Step::Name => div()
                .flex_1()
                .min_h_0()
                .px_3()
                .py_3()
                .text_sm()
                .text_color(colors.text_muted)
                .child("This becomes the new folder and workspace name.")
                .into_any_element(),
            Step::Parent => div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(colors.text)
                        .child(format!("New folder: {}/", self.name)),
                )
                .child(self.render_parent_results(&colors, cx))
                .into_any_element(),
        };
        let query = &self.query;
        let placeholder = if self.step == Step::Name {
            "e.g. api-redesign"
        } else {
            "Filter folders under ~"
        };
        let hint = if self.step == Step::Name {
            "↵ next  ·  tab next  ·  esc cancel"
        } else {
            "↑↓ choose  ·  ↵ create  ·  esc back"
        };
        scrim("workspace-create-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkspaceCreateEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("WorkspaceCreate")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(crate::palette::optional_title(title, &colors))
                    .child(query_row(query, placeholder, true, &colors).into_any_element())
                    .child(body)
                    .child(hint_row(hint, &colors)),
            )
    }
}

fn display_path(path: &std::path::Path) -> String {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return path.display().to_string();
    };
    path.strip_prefix(&home)
        .map(|relative| format!("~/{}", relative.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

impl_palette_query_input!(WorkspaceCreateView);
