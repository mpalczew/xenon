//! Keyboard-first creator for a new directory-backed workspace.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, AppContext, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Task, Window, div,
};
use nucleo::{Config, Matcher};
use theme::ActiveTheme;
use xenon_design_system::{PaletteOverlay, palette_overlay};

use crate::palette::{
    DetailRow, PaletteLayout, ScrollResults, detail_row, fuzzy_index_order, hint_row,
    reveal_selected, scroll_results,
};
use crate::workspace_discover::{
    discover_parent_dirs, expand_user_path, list_parent_candidates, path_is_dir,
    resolve_existing_dir,
};

const PARENT_LIST_DEBOUNCE: Duration = Duration::from_millis(60);
const PARENT_EMPTY_TAKE: usize = 12;
const PARENT_FILTER_TAKE: usize = 20;

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
    input: gpui::Entity<xenon_design_system::TextInputView>,
    _input_sub: gpui::Subscription,
    matcher: Matcher,
    scroll: ScrollHandle,
    /// Path-scoped listing (`~/src`); `None` means fuzzy-filter `parents`.
    scoped: Option<Vec<PathBuf>>,
    listing_gen: u64,
    _parents_task: Option<Task<()>>,
    _listing_task: Option<Task<()>>,
}

impl EventEmitter<WorkspaceCreateEvent> for WorkspaceCreateView {}

impl WorkspaceCreateView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            xenon_design_system::TextInputView::new(
                xenon_design_system::TextInputConfig::single_line("e.g. api-redesign")
                    .parent_navigation()
                    .appearance(xenon_design_system::TextInputAppearance::Palette),
                cx,
            )
        });
        input.update(cx, |input, cx| input.open(cx));
        let focus = input.read(cx).focus_handle();
        let input_sub = cx.subscribe(&input, |this, _, event, cx| {
            if let xenon_design_system::TextInputEvent::Changed(query) = event {
                this.set_query(query.clone(), cx);
            }
        });
        let mut view = Self {
            step: Step::Name,
            name: String::new(),
            query: String::new(),
            parents: Vec::new(),
            results: Vec::new(),
            selected: 0,
            focus,
            input,
            _input_sub: input_sub,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
            scoped: None,
            listing_gen: 0,
            _parents_task: None,
            _listing_task: None,
        };
        view.load_parent_dirs(cx);
        view
    }

    fn load_parent_dirs(&mut self, cx: &mut Context<Self>) {
        self._parents_task = Some(cx.spawn(async move |this, cx| {
            let dirs = cx
                .background_executor()
                .spawn(async { discover_parent_dirs() })
                .await;
            this.update(cx, |this, cx| {
                this.parents = dirs;
                if this.scoped.is_none() {
                    this.refilter();
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn refilter(&mut self) {
        if let Some(scoped) = &self.scoped {
            self.results = scoped.iter().take(PARENT_FILTER_TAKE).cloned().collect();
            self.selected = 0;
            return;
        }
        let q = self.query.trim();
        self.results = if q.is_empty() {
            self.parents
                .iter()
                .take(PARENT_EMPTY_TAKE)
                .cloned()
                .collect()
        } else {
            let haystacks: Vec<String> = self
                .parents
                .iter()
                .map(|path| parent_haystack(path))
                .collect();
            fuzzy_index_order(&haystacks, q, &mut self.matcher)
                .into_iter()
                .map(|i| self.parents[i].clone())
                .take(PARENT_FILTER_TAKE)
                .collect()
        };
        self.selected = 0;
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        if self.step == Step::Parent {
            self.kick_parent_listing(cx);
        }
        cx.notify();
    }

    fn kick_parent_listing(&mut self, cx: &mut Context<Self>) {
        let q = self.query.trim().to_string();
        let Some(expanded) = expand_user_path(&q) else {
            self.listing_gen = self.listing_gen.wrapping_add(1);
            self._listing_task = None;
            self.scoped = None;
            self.refilter();
            return;
        };
        if path_is_dir(&expanded) {
            self.scoped = Some(vec![expanded.clone()]);
            self.results = vec![expanded];
            self.selected = 0;
        } else {
            self.scoped = Some(Vec::new());
            self.results.clear();
            self.selected = 0;
        }
        self.listing_gen = self.listing_gen.wrapping_add(1);
        let token = self.listing_gen;
        let query_snapshot = q;
        self._listing_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PARENT_LIST_DEBOUNCE).await;
            let still = this
                .update(cx, |this, _| {
                    this.listing_gen == token && this.query.trim() == query_snapshot
                })
                .unwrap_or(false);
            if !still {
                return;
            }
            let snapshot = query_snapshot.clone();
            let found = cx
                .background_executor()
                .spawn(async move { list_parent_candidates(&snapshot) })
                .await;
            this.update(cx, |this, cx| {
                if this.listing_gen != token || this.query.trim() != query_snapshot {
                    return;
                }
                this.scoped = found;
                this.refilter();
                cx.notify();
            })
            .ok();
        }));
    }

    fn selected_parent(&self) -> Option<PathBuf> {
        self.results.get(self.selected).cloned()
    }

    fn parent_to_create(&self) -> Option<PathBuf> {
        self.selected_parent()
            .filter(|p| path_is_dir(p))
            .or_else(|| expand_user_path(&self.query).and_then(|p| resolve_existing_dir(&p)))
    }

    fn valid_name_query(&self) -> bool {
        is_valid_workspace_name(&self.query)
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.step == Step::Name && self.valid_name_query() {
            self.name = self.query.trim().to_string();
            self.step = Step::Parent;
            self.query.clear();
            self.input.update(cx, |input, cx| {
                input.set_text("", cx);
                input.set_placeholder("e.g. ~/src", cx);
            });
            self.scoped = None;
            self.selected = 0;
            self.refilter();
            cx.notify();
        }
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Name => self.advance(cx),
            Step::Parent => {
                let Some(parent) = self.parent_to_create() else {
                    return;
                };
                if is_valid_workspace_name(&self.name) {
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
                self.input.update(cx, |input, cx| {
                    input.set_text("", cx);
                    input.set_placeholder("e.g. api-redesign", cx);
                });
                cx.notify();
            }
            "escape" => cx.emit(WorkspaceCreateEvent::Dismissed),
            "enter" => self.confirm(cx),
            "tab" if self.step == Step::Name => self.advance(cx),
            "up" if self.step == Step::Parent => self.move_selection(-1, cx),
            "down" if self.step == Step::Parent => self.move_selection(1, cx),
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let hint = if self.step == Step::Name {
            "↵ next  ·  tab next  ·  esc cancel"
        } else {
            "↑↓ choose  ·  ↵ create  ·  esc back"
        };
        palette_overlay(
            PaletteOverlay {
                id: "workspace-create-scrim",
                layout,
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "WorkspaceCreate",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(WorkspaceCreateEvent::Dismissed),
                children: vec![
                    crate::palette::optional_title(title, &colors).into_any_element(),
                    self.input.clone().into_any_element(),
                    body.into_any_element(),
                    hint_row(hint, &colors).into_any_element(),
                ],
            },
            cx,
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

fn parent_haystack(path: &std::path::Path) -> String {
    format!("{}\n{}", path.display(), display_path(path))
}

fn is_valid_workspace_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::is_valid_workspace_name;

    #[test]
    fn name_rejects_paths() {
        assert!(is_valid_workspace_name("crypto"));
        assert!(!is_valid_workspace_name(""));
        assert!(!is_valid_workspace_name("~/src"));
        assert!(!is_valid_workspace_name("foo/bar"));
    }
}
