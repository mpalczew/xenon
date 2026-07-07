//! `XeroApp`: the window root. Owns the workspace registry, the active
//! terminal/editor, and the finder overlay; wires the shell's actions.

use std::path::PathBuf;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Render, Styled, Subscription, Window, div,
};
use theme::ActiveTheme;
use xero_core::{Active, Registry, WorkspaceId, WorkspaceRec};
use xero_editor::EditorView;
use xero_terminal::TerminalView;

use crate::finder::{FinderEvent, FinderView};
use crate::{AddWorkspace, FilePalette, OpenFile, ToggleSidebar};

pub struct XeroApp {
    registry: Registry,
    active: Option<WorkspaceId>,
    terminal: Option<Entity<TerminalView>>,
    editor: Option<Entity<EditorView>>,
    finder: Option<Entity<FinderView>>,
    sidebar_collapsed: bool,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
}

impl XeroApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let registry = xero_store::load_registry().unwrap_or_default();
        let active = registry
            .active
            .map(|a| a.workspace)
            .or_else(|| registry.workspaces.first().map(|w| w.id));
        let mut app = Self {
            registry,
            active: None,
            terminal: None,
            editor: None,
            finder: None,
            sidebar_collapsed: false,
            focus: cx.focus_handle(),
            _finder_sub: None,
        };
        if let Some(id) = active {
            app.activate_workspace(id, cx);
        }
        app
    }

    pub(crate) fn registry(&self) -> &Registry {
        &self.registry
    }

    pub(crate) fn active_workspace(&self) -> Option<WorkspaceId> {
        self.active
    }

    pub(crate) fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed
    }

    fn active_root(&self) -> Option<PathBuf> {
        let id = self.active?;
        self.registry.workspace(id).map(|w| w.root.clone())
    }

    pub(crate) fn activate_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        let Some(root) = self.registry.workspace(id).map(|w| w.root.clone()) else {
            return;
        };
        self.active = Some(id);
        self.editor = None;
        self.finder = None;
        self.terminal = Some(cx.new(|cx| TerminalView::new(Some(root), cx)));
        self.persist_active();
        cx.notify();
    }

    fn add_workspace(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.register_workspace(path, cx)).ok();
            }
        })
        .detach();
    }

    fn register_workspace(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        let record = WorkspaceRec::new(root);
        let id = record.id;
        self.registry.workspaces.push(record);
        let _ = xero_store::save_registry(&self.registry);
        self.activate_workspace(id, cx);
    }

    fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.open_editor(path, cx)).ok();
            }
        })
        .detach();
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match EditorView::build(path, cx) {
            Ok(editor) => self.editor = Some(editor),
            Err(error) => log::error!("open failed: {error}"),
        }
        self.finder = None;
        cx.notify();
    }

    fn open_palette(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.active_root() else {
            return;
        };
        let finder = cx.new(|cx| FinderView::new(root, cx));
        self._finder_sub = Some(cx.subscribe(&finder, Self::on_finder_event));
        self.finder = Some(finder);
        cx.notify();
    }

    fn on_finder_event(
        &mut self,
        _finder: Entity<FinderView>,
        event: &FinderEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FinderEvent::Selected(relative) => {
                if let Some(root) = self.active_root() {
                    self.open_editor(root.join(relative), cx);
                }
            }
            FinderEvent::Dismissed => {
                self.finder = None;
                cx.notify();
            }
        }
    }

    fn persist_active(&self) {
        if let Some(workspace) = self.active
            && let Some(record) = self.registry.workspace(workspace)
            && let Some(&stream) = record.streams.first()
        {
            let mut registry = self.registry.clone();
            registry.active = Some(Active { workspace, stream });
            let _ = xero_store::save_registry(&registry);
        }
    }
}

impl Focusable for XeroApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for XeroApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let sidebar = (!self.sidebar_collapsed).then(|| self.render_sidebar(cx));
        let main = self.render_main(cx);
        let finder = self.finder.clone();
        div()
            .track_focus(&self.focus)
            .key_context("XeroApp")
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_collapsed = !this.sidebar_collapsed;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.open_file_dialog(cx)))
            .on_action(cx.listener(|this, _: &AddWorkspace, _, cx| this.add_workspace(cx)))
            .on_action(cx.listener(|this, _: &FilePalette, _, cx| this.open_palette(cx)))
            .relative()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .child(div().flex().size_full().children(sidebar).child(main))
            .children(finder)
    }
}

impl XeroApp {
    fn render_main(&self, _cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut panel = div().flex().flex_1().size_full();
        match (&self.terminal, &self.editor) {
            (Some(terminal), Some(editor)) => {
                panel = panel
                    .child(div().flex_1().child(terminal.clone()))
                    .child(div().flex_1().child(editor.clone()));
            }
            (Some(terminal), None) => {
                panel = panel.child(div().flex_1().child(terminal.clone()));
            }
            _ => {
                panel = panel.items_center().justify_center().child("Add a workspace to begin");
            }
        }
        panel
    }
}
