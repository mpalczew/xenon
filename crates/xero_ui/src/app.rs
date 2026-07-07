//! `XeroApp`: the window root. Owns the workspace registry and, per stream, a
//! live terminal + editor. Streams are the unit of switching: each workspace
//! holds one or more, and every stream keeps its own running PTY.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Render, Styled, Subscription, Task, Window, div,
};
use theme::ActiveTheme;
use xero_core::{Active, Registry, Stream, StreamId, WorkspaceId, WorkspaceRec};
use xero_editor::EditorView;
use xero_ide::{IdeCommand, IdeServer};
use xero_terminal::{TerminalEvent, TerminalView};

use crate::finder::{FinderEvent, FinderView};
use crate::{
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, OpenFile,
    ResetFontSize, ToggleSidebar,
};

/// The open editor tabs for one stream, plus which is focused.
#[derive(Default)]
pub(crate) struct EditorStack {
    pub tabs: Vec<EditorTab>,
    pub active: usize,
}

pub(crate) struct EditorTab {
    pub path: PathBuf,
    pub name: String,
    pub view: Entity<EditorView>,
}

pub struct XeroApp {
    registry: Registry,
    // Stream metadata (name/session) and, per stream, the live views. Keeping
    // terminals here means switching streams never tears down a running PTY.
    streams: HashMap<StreamId, Stream>,
    terminals: HashMap<StreamId, Entity<TerminalView>>,
    // Per stream, a stack of open editor tabs.
    editors: HashMap<StreamId, EditorStack>,
    active: Option<StreamId>,
    finder: Option<Entity<FinderView>>,
    sidebar_collapsed: bool,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
    // Streams whose terminal rang the bell while unfocused (agent wants
    // attention); shown as a dot in the sidebar, cleared when the stream opens.
    attention: HashSet<StreamId>,
    _bell_subs: Vec<Subscription>,
    // IDE server: agents in the terminal connect here to drive xero. Its env is
    // injected into every terminal so Claude Code discovers it.
    ide: Option<IdeServer>,
    _ide_task: Option<Task<()>>,
}

impl XeroApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let registry = xero_store::load_registry().unwrap_or_default();
        let mut app = Self {
            registry,
            streams: HashMap::new(),
            terminals: HashMap::new(),
            editors: HashMap::new(),
            active: None,
            finder: None,
            sidebar_collapsed: false,
            focus: cx.focus_handle(),
            _finder_sub: None,
            attention: HashSet::new(),
            _bell_subs: Vec::new(),
            ide: None,
            _ide_task: None,
        };
        app.load_streams();
        app.start_ide_server(cx);
        let active = app
            .registry
            .active
            .map(|a| a.stream)
            .filter(|s| app.streams.contains_key(s))
            .or_else(|| app.first_stream());
        if let Some(id) = active {
            app.activate_stream(id, cx);
        }
        app
    }

    /// Populate the stream metadata map from the store, synthesizing a default
    /// stream for any workspace that has none.
    fn load_streams(&mut self) {
        let mut dirty = false;
        for workspace in &mut self.registry.workspaces {
            if workspace.streams.is_empty() {
                let stream = Stream::new("main");
                workspace.streams.push(stream.id);
                let _ = xero_store::save_session(workspace.id, &stream);
                self.streams.insert(stream.id, stream);
                dirty = true;
                continue;
            }
            for &id in &workspace.streams {
                let stream = xero_store::load_session(workspace.id, id)
                    .unwrap_or_else(|_| synthesize_stream(id));
                self.streams.insert(id, stream);
            }
        }
        if dirty {
            let _ = xero_store::save_registry(&self.registry);
        }
    }

    /// Start the IDE server over all workspace roots and consume its commands
    /// (openFile) on the UI thread.
    fn start_ide_server(&mut self, cx: &mut Context<Self>) {
        let roots: Vec<_> = self.registry.workspaces.iter().map(|w| w.root.clone()).collect();
        let (tx, rx) = async_channel::unbounded();
        match IdeServer::start(roots, tx) {
            Ok(server) => self.ide = Some(server),
            Err(error) => {
                log::error!("IDE server failed to start: {error}");
                return;
            }
        }
        self._ide_task = Some(cx.spawn(async move |view, cx| {
            while let Ok(command) = rx.recv().await {
                if view.update(cx, |app, cx| app.handle_ide(command, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn handle_ide(&mut self, command: IdeCommand, cx: &mut Context<Self>) {
        match command {
            IdeCommand::OpenFile(path) => {
                log::info!("IDE openFile: {}", path.display());
                self.open_editor(path, false, cx); // don't steal terminal focus
            }
        }
    }

    /// Environment injected into every terminal so agents find the IDE server.
    fn terminal_env(&self) -> Vec<(String, String)> {
        self.ide.as_ref().map(|server| server.env()).unwrap_or_default()
    }

    pub(crate) fn registry(&self) -> &Registry {
        &self.registry
    }

    pub(crate) fn active_stream(&self) -> Option<StreamId> {
        self.active
    }

    pub(crate) fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed
    }

    pub(crate) fn stream_name(&self, id: StreamId) -> &str {
        self.streams.get(&id).map(|s| s.name.as_str()).unwrap_or("stream")
    }

    fn first_stream(&self) -> Option<StreamId> {
        self.registry.workspaces.iter().find_map(|w| w.streams.first().copied())
    }

    fn workspace_of(&self, stream: StreamId) -> Option<&WorkspaceRec> {
        self.registry.workspaces.iter().find(|w| w.streams.contains(&stream))
    }

    fn stream_root(&self, stream: StreamId) -> Option<PathBuf> {
        let workspace = self.workspace_of(stream)?;
        let record = self.streams.get(&stream)?;
        Some(record.working_dir(&workspace.root))
    }

    pub(crate) fn activate_stream(&mut self, id: StreamId, cx: &mut Context<Self>) {
        let Some(root) = self.stream_root(id) else {
            return;
        };
        self.active = Some(id);
        self.finder = None;
        self.attention.remove(&id);
        if !self.terminals.contains_key(&id) {
            let env = self.terminal_env();
            let terminal = cx.new(|cx| TerminalView::new(Some(root), env, cx));
            self._bell_subs.push(cx.subscribe(&terminal, move |this, _view, event, cx| {
                match event {
                    TerminalEvent::Bell => this.on_bell(id, cx),
                }
            }));
            self.terminals.insert(id, terminal);
        }
        self.persist_active();
        cx.notify();
    }

    /// A background stream rang the bell: flag it for the sidebar. The active
    /// stream is assumed watched, so it isn't flagged.
    fn on_bell(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.active != Some(id) {
            self.attention.insert(id);
            cx.notify();
        }
    }

    pub(crate) fn needs_attention(&self, id: StreamId) -> bool {
        self.attention.contains(&id)
    }

    /// Switch to a stream and focus its terminal, so keyboard focus lands
    /// somewhere definite on every switch (called from the sidebar click).
    pub(crate) fn select_stream(
        &mut self,
        id: StreamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_stream(id, cx);
        if let Some(terminal) = self.active.and_then(|id| self.terminals.get(&id)) {
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
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
        let mut record = WorkspaceRec::new(root);
        let stream = Stream::new("main");
        let stream_id = stream.id;
        record.streams.push(stream_id);
        let workspace_id = record.id;
        self.registry.workspaces.push(record);
        self.streams.insert(stream_id, stream.clone());
        let _ = xero_store::save_session(workspace_id, &stream);
        let _ = xero_store::save_registry(&self.registry);
        self.activate_stream(stream_id, cx);
    }

    pub(crate) fn add_stream(&mut self, workspace: WorkspaceId, cx: &mut Context<Self>) {
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        let stream = Stream::new(format!("stream {}", record.streams.len() + 1));
        let stream_id = stream.id;
        record.streams.push(stream_id);
        self.streams.insert(stream_id, stream.clone());
        let _ = xero_store::save_session(workspace, &stream);
        let _ = xero_store::save_registry(&self.registry);
        self.activate_stream(stream_id, cx);
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
                this.update(cx, |this, cx| this.open_editor(path, true, cx)).ok();
            }
        })
        .detach();
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, focus: bool, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            log::warn!("open_editor: no active stream for {}", path.display());
            return;
        };
        let stack = self.editors.entry(id).or_default();
        // Focus an already-open tab rather than opening a duplicate.
        if let Some(index) = stack.tabs.iter().position(|tab| tab.path == path) {
            stack.active = index;
        } else {
            match EditorView::build(path.clone(), focus, cx) {
                Ok(view) => {
                    let name = file_name(&path);
                    let stack = self.editors.entry(id).or_default();
                    stack.tabs.push(EditorTab { path, name, view });
                    stack.active = stack.tabs.len() - 1;
                }
                Err(error) => log::error!("open failed: {error}"),
            }
        }
        self.finder = None;
        cx.notify();
    }

    /// The open tabs and focused index for the active stream.
    pub(crate) fn editor_stack(&self) -> Option<&EditorStack> {
        self.active.and_then(|id| self.editors.get(&id))
    }

    pub(crate) fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.active
            && let Some(stack) = self.editors.get_mut(&id)
            && index < stack.tabs.len()
        {
            stack.active = index;
            cx.notify();
        }
    }

    /// Close the tab at `index` in the active stream; drops the stack when empty.
    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.active
            && let Some(stack) = self.editors.get_mut(&id)
            && index < stack.tabs.len()
        {
            stack.tabs.remove(index);
            if stack.tabs.is_empty() {
                self.editors.remove(&id);
            } else {
                stack.active = stack.active.min(stack.tabs.len() - 1);
            }
            cx.notify();
        }
    }

    /// Close the focused tab (Cmd-W / toolbar).
    pub(crate) fn close_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(stack) = self.editor_stack() {
            let active = stack.active;
            self.close_tab(active, cx);
        }
    }

    pub(crate) fn has_editor(&self) -> bool {
        self.editor_stack().is_some_and(|stack| !stack.tabs.is_empty())
    }

    fn open_palette(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.active.and_then(|id| self.stream_root(id)) else {
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
                if let Some(root) = self.active.and_then(|id| self.stream_root(id)) {
                    self.open_editor(root.join(relative), true, cx);
                }
            }
            FinderEvent::Dismissed => {
                self.finder = None;
                cx.notify();
            }
        }
    }

    fn persist_active(&self) {
        if let Some(stream) = self.active
            && let Some(workspace) = self.workspace_of(stream)
        {
            let mut registry = self.registry.clone();
            registry.active = Some(Active { workspace: workspace.id, stream });
            let _ = xero_store::save_registry(&registry);
        }
    }
}

/// A placeholder stream for an id whose session file is missing or corrupt.
fn synthesize_stream(id: StreamId) -> Stream {
    Stream { id, name: "main".into(), backing: Default::default(), session: Default::default() }
}

impl Focusable for XeroApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for XeroApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let toolbar = self.render_toolbar(cx);
        let sidebar = (!self.sidebar_collapsed).then(|| self.render_sidebar(cx));
        let main = self.render_main(window, cx);
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
            .on_action(cx.listener(|this, _: &CloseEditor, _, cx| this.close_editor(cx)))
            .on_action(cx.listener(|_, _: &IncreaseFontSize, window, cx| {
                xero_settings::adjust_font_size(cx, 1.0);
                window.refresh();
            }))
            .on_action(cx.listener(|_, _: &DecreaseFontSize, window, cx| {
                xero_settings::adjust_font_size(cx, -1.0);
                window.refresh();
            }))
            .on_action(cx.listener(|_, _: &ResetFontSize, window, cx| {
                xero_settings::reset_font_size(cx);
                window.refresh();
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .child(toolbar)
            .child(div().flex().flex_1().min_h_0().children(sidebar).child(main))
            .children(finder)
    }
}

impl XeroApp {
    fn render_main(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let terminal = self.active.and_then(|id| self.terminals.get(&id)).cloned();
        let active_view = self
            .editor_stack()
            .and_then(|stack| stack.tabs.get(stack.active))
            .map(|tab| tab.view.clone());

        let ring = |focused: bool| {
            if focused { colors.border_focused } else { gpui::transparent_black() }
        };
        let term_focused = terminal
            .as_ref()
            .is_some_and(|t| t.read(cx).focus_handle(cx).contains_focused(window, cx));
        let editor_focused = active_view
            .as_ref()
            .is_some_and(|e| e.read(cx).focus_handle(cx).contains_focused(window, cx));

        let tab_bar = self.has_editor().then(|| self.render_tab_bar(cx));
        let mut panel = div().flex().flex_1().size_full();
        match (terminal, active_view) {
            (Some(terminal), Some(view)) => {
                panel = panel
                    .child(
                        div()
                            .flex_1()
                            .border_2()
                            .border_color(ring(term_focused))
                            .child(terminal),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .border_2()
                            .border_color(ring(editor_focused))
                            .children(tab_bar)
                            .child(div().flex_1().min_h_0().child(view)),
                    );
            }
            (Some(terminal), None) => {
                panel = panel.child(div().flex_1().child(terminal));
            }
            _ => {
                panel = panel.items_center().justify_center().child("Add a workspace to begin");
            }
        }
        panel
    }
}

/// The display name for an editor tab (file name, or the path if none).
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
