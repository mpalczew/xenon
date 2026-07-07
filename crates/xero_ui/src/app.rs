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
use crate::rename::{RenameEvent, RenameView};
use crate::{
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, OpenFile,
    ResetFontSize, ToggleSidebar,
};

/// The terminals open in one stream, as tabs, plus which is focused.
#[derive(Default)]
pub(crate) struct TerminalStack {
    pub tabs: Vec<Entity<TerminalView>>,
    pub active: usize,
}

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
    // Per stream: a stack of terminal tabs and a stack of editor tabs.
    terminals: HashMap<StreamId, TerminalStack>,
    editors: HashMap<StreamId, EditorStack>,
    active: Option<StreamId>,
    finder: Option<Entity<FinderView>>,
    sidebar_collapsed: bool,
    // Workspaces whose streams are hidden in the sidebar.
    collapsed_workspaces: HashSet<WorkspaceId>,
    // The stream currently being renamed inline, plus its editing field.
    renaming: Option<(StreamId, Entity<RenameView>)>,
    _rename_sub: Option<Subscription>,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
    // Streams whose terminal rang the bell while unfocused (agent wants
    // attention); shown as a dot in the sidebar, cleared when the stream opens.
    attention: HashSet<StreamId>,
    _bell_subs: Vec<Subscription>,
    // Foreground state, so bells only raise a desktop notification when xero is
    // in the background (the sidebar dot covers the foreground case).
    window_active: bool,
    _activation_sub: Option<Subscription>,
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
            collapsed_workspaces: HashSet::new(),
            renaming: None,
            _rename_sub: None,
            focus: cx.focus_handle(),
            _finder_sub: None,
            attention: HashSet::new(),
            _bell_subs: Vec::new(),
            window_active: true,
            _activation_sub: None,
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
        if !self.terminals.contains_key(&id) {
            let terminal = self.spawn_terminal(root, id, cx);
            self.terminals.insert(id, TerminalStack { tabs: vec![terminal], active: 0 });
        }
        self.persist_active();
        cx.notify();
    }

    /// Create a terminal for `stream` at `root` and wire its bell/interaction
    /// events to the stream's attention flag.
    fn spawn_terminal(
        &mut self,
        root: PathBuf,
        stream: StreamId,
        cx: &mut Context<Self>,
    ) -> Entity<TerminalView> {
        let env = self.terminal_env();
        let terminal = cx.new(|cx| TerminalView::new(Some(root), env, cx));
        self._bell_subs.push(cx.subscribe(&terminal, move |this, _view, event, cx| match event {
            TerminalEvent::Bell | TerminalEvent::Finished => this.flag_attention(stream, cx),
            TerminalEvent::Interacted => this.clear_attention(stream, cx),
        }));
        terminal
    }

    /// Add another terminal tab to the active stream.
    pub(crate) fn add_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(root) = self.stream_root(id) else {
            return;
        };
        let terminal = self.spawn_terminal(root, id, cx);
        let stack = self.terminals.entry(id).or_default();
        stack.tabs.push(terminal);
        stack.active = stack.tabs.len() - 1;
        cx.notify();
    }

    pub(crate) fn terminal_stack(&self) -> Option<&TerminalStack> {
        self.active.and_then(|id| self.terminals.get(&id))
    }

    fn active_terminal(&self) -> Option<Entity<TerminalView>> {
        self.terminal_stack().and_then(|s| s.tabs.get(s.active)).cloned()
    }

    pub(crate) fn activate_terminal_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let terminal = {
            let Some(stack) = self.terminals.get_mut(&id) else {
                return;
            };
            if index >= stack.tabs.len() {
                return;
            }
            stack.active = index;
            stack.tabs[index].clone()
        };
        terminal.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Close a terminal tab, keeping at least one terminal per stream, and move
    /// focus to the terminal that becomes active.
    pub(crate) fn close_terminal_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let survivor = {
            let Some(stack) = self.terminals.get_mut(&id) else {
                return;
            };
            if index >= stack.tabs.len() || stack.tabs.len() == 1 {
                return;
            }
            stack.tabs.remove(index);
            stack.active = stack.active.min(stack.tabs.len() - 1);
            stack.tabs[stack.active].clone()
        };
        survivor.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// The stream's agent rang the bell: flag it (even when active/focused). The
    /// flag stays until the user acts in that terminal (click/type/scroll). Also
    /// raises a macOS notification when xero is in the background.
    fn flag_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.attention.insert(id) {
            if !self.window_active {
                self.post_notification(id);
            }
            cx.notify();
        }
    }

    /// Show a native macOS notification via osascript (fire-and-forget).
    fn post_notification(&self, id: StreamId) {
        let stream = self.stream_name(id).to_string();
        let workspace = self.workspace_of(id).map(|w| w.name.clone()).unwrap_or_default();
        let body = format!("{workspace} / {stream}").replace(['"', '\\'], "");
        let script =
            format!("display notification \"{body}\" with title \"xero\" subtitle \"Agent finished\"");
        let _ = std::process::Command::new("osascript").arg("-e").arg(script).spawn();
    }

    fn clear_attention(&mut self, id: StreamId, cx: &mut Context<Self>) {
        if self.attention.remove(&id) {
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
        if let Some(terminal) = self.active_terminal() {
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
        let stream = Stream::new("stream 1");
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

    /// Move `dragged` to `target`'s position within their shared workspace.
    /// No-op across workspaces (drag reorders within one workspace only).
    pub(crate) fn reorder_stream(
        &mut self,
        dragged: StreamId,
        target: StreamId,
        cx: &mut Context<Self>,
    ) {
        if dragged == target {
            return;
        }
        let Some(workspace) = self.workspace_of(target).map(|w| w.id) else {
            return;
        };
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        let Some(from) = record.streams.iter().position(|&s| s == dragged) else {
            return;
        };
        record.streams.remove(from);
        let to = record.streams.iter().position(|&s| s == target).unwrap_or(record.streams.len());
        record.streams.insert(to, dragged);
        let _ = xero_store::save_registry(&self.registry);
        cx.notify();
    }

    /// Move workspace `dragged` to `target`'s position in the sidebar.
    pub(crate) fn reorder_workspace(
        &mut self,
        dragged: WorkspaceId,
        target: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        if dragged == target {
            return;
        }
        let list = &mut self.registry.workspaces;
        let Some(from) = list.iter().position(|w| w.id == dragged) else {
            return;
        };
        let record = list.remove(from);
        let to = list.iter().position(|w| w.id == target).unwrap_or(list.len());
        list.insert(to, record);
        let _ = xero_store::save_registry(&self.registry);
        cx.notify();
    }

    pub(crate) fn toggle_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if !self.collapsed_workspaces.insert(id) {
            self.collapsed_workspaces.remove(&id);
        }
        cx.notify();
    }

    pub(crate) fn is_workspace_collapsed(&self, id: WorkspaceId) -> bool {
        self.collapsed_workspaces.contains(&id)
    }

    /// Begin renaming a stream: open an inline field seeded with its name.
    pub(crate) fn start_rename(&mut self, id: StreamId, cx: &mut Context<Self>) {
        let name = self.stream_name(id).to_string();
        let field = cx.new(|cx| RenameView::new(name, cx));
        self._rename_sub = Some(cx.subscribe(&field, move |this, _field, event, cx| match event {
            RenameEvent::Committed(name) => this.apply_rename(id, name.clone(), cx),
            RenameEvent::Cancelled => this.cancel_rename(cx),
        }));
        self.renaming = Some((id, field));
        cx.notify();
    }

    /// The inline rename field for `id`, if that stream is being renamed.
    pub(crate) fn rename_field(&self, id: StreamId) -> Option<Entity<RenameView>> {
        self.renaming.as_ref().filter(|(target, _)| *target == id).map(|(_, field)| field.clone())
    }

    fn apply_rename(&mut self, id: StreamId, name: String, cx: &mut Context<Self>) {
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.name = name;
            if let Some(workspace) = self.workspace_of(id).map(|w| w.id) {
                let _ = xero_store::save_session(workspace, &self.streams[&id]);
            }
        }
        self.cancel_rename(cx);
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming = None;
        self._rename_sub = None;
        cx.notify();
    }

    /// Close a stream: drop its terminals/editors, delete its session, and remove
    /// it from its workspace. Refuses to remove a workspace's last stream.
    pub(crate) fn close_stream(
        &mut self,
        id: StreamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.workspace_of(id).map(|w| w.id) else {
            return;
        };
        let Some(record) = self.registry.workspace_mut(workspace) else {
            return;
        };
        if record.streams.len() <= 1 {
            return;
        }
        record.streams.retain(|&stream| stream != id);
        let fallback = record.streams.first().copied();
        self.streams.remove(&id);
        self.terminals.remove(&id);
        self.editors.remove(&id);
        self.attention.remove(&id);
        let _ = xero_store::delete_session(workspace, id);
        let _ = xero_store::save_registry(&self.registry);
        if self.active == Some(id) {
            self.active = None;
            if let Some(next) = fallback {
                self.select_stream(next, window, cx);
            }
        }
        cx.notify();
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
        if self._activation_sub.is_none() {
            self.window_active = window.is_window_active();
            self._activation_sub = Some(cx.observe_window_activation(window, |this, window, _| {
                this.window_active = window.is_window_active();
            }));
        }
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
        let terminal = self.active_terminal();
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

        let terminal_tabs = terminal.is_some().then(|| self.render_terminal_tabs(cx));
        let editor_tabs = self.has_editor().then(|| self.render_tab_bar(cx));
        let terminal_pane = terminal.map(|terminal| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .min_w_0()
                .border_2()
                .border_color(ring(term_focused))
                .children(terminal_tabs)
                .child(div().flex_1().min_h_0().child(terminal))
        });

        let mut panel = div().flex().flex_1().size_full();
        match (terminal_pane, active_view) {
            (Some(terminal_pane), Some(view)) => {
                panel = panel.child(terminal_pane).child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .border_2()
                        .border_color(ring(editor_focused))
                        .children(editor_tabs)
                        .child(div().flex_1().min_h_0().child(view)),
                );
            }
            (Some(terminal_pane), None) => {
                panel = panel.child(terminal_pane);
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
