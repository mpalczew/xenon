//! `XeroApp`: the window root. Owns the workspace registry and, per stream, a
//! live terminal + editor. Streams are the unit of switching: each workspace
//! holds one or more, and every stream keeps its own running PTY.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Task, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, px, size,
};
use theme::ActiveTheme;
use xero_core::{Active, Layout, Registry, Stream, StreamId, WorkspaceId, WorkspaceRec};
use xero_editor::EditorView;
use xero_finder::{FileIndex, Finder};
use xero_ide::{IdeCommand, IdeServer};
use xero_terminal::{TerminalEvent, TerminalView};

use crate::file_browser::{FileBrowser, TreeRow, dir_marker, file_icon};
use crate::finder::{FinderEvent, FinderView};
use crate::rename::{RenameEvent, RenameView};
use crate::settings::SettingsView;
use crate::{
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, OpenFile,
    ResetFontSize, ToggleBrowser, ToggleEditor, ToggleSettings, ToggleSidebar, ToggleTerminal,
};

mod browser;
mod editors;
mod navigation;
mod panels;
mod render;
mod streams;
mod terminals;
mod workspaces;

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

/// Which pane held keyboard focus before the finder opened, so Escape can
/// return focus there instead of dropping it into the void.
#[derive(Clone, Copy)]
enum FocusPane {
    Terminal,
    Editor,
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
    // Per-root fuzzy index shared by cmd-p and cmd-click resolution, built off
    // the UI thread. `index_tasks` keeps in-flight builds alive, keyed by root so
    // a new build for the same root replaces (cancels) the previous one.
    file_indexes: HashMap<PathBuf, Arc<FileIndex>>,
    index_tasks: HashMap<PathBuf, Task<()>>,
    // The pane focused when the finder opened, and (on dismiss) the pane to
    // re-focus during the next render (render is where a `Window` is available).
    restore_pane: Option<FocusPane>,
    pending_focus: Option<FocusPane>,
    // A cmd-clicked name to open cmd-p with, deferred to render (which has a
    // Window) from the windowless terminal-event subscription.
    pending_palette_query: Option<String>,
    sidebar_collapsed: bool,
    terminal_collapsed: bool,
    editor_collapsed: bool,
    // Workspaces whose streams are hidden in the sidebar.
    collapsed_workspaces: HashSet<WorkspaceId>,
    // Closed-workspace list is collapsed by default (archive, not peer list).
    closed_section_collapsed: bool,
    file_browser: FileBrowser,
    /// Dedicated settings window (cmd-,). None when closed or not yet opened.
    settings_window: Option<WindowHandle<SettingsView>>,
    // The stream currently being renamed inline, plus its editing field.
    renaming: Option<(StreamId, Entity<RenameView>)>,
    _rename_sub: Option<Subscription>,
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
        let settings = xero_store::load_settings().unwrap_or_default();
        xero_settings::apply(&settings, cx);
        xero_terminal::apply_theme(cx);
        let mut app = Self {
            registry,
            streams: HashMap::new(),
            terminals: HashMap::new(),
            editors: HashMap::new(),
            active: None,
            finder: None,
            file_indexes: HashMap::new(),
            index_tasks: HashMap::new(),
            restore_pane: None,
            pending_focus: None,
            pending_palette_query: None,
            sidebar_collapsed: false,
            terminal_collapsed: false,
            editor_collapsed: false,
            collapsed_workspaces: HashSet::new(),
            closed_section_collapsed: true,
            file_browser: FileBrowser::default(),
            settings_window: None,
            renaming: None,
            _rename_sub: None,
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
                save_session(workspace.id, &stream, "load_streams default stream");
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
            save_registry(&self.registry, "load_streams default stream");
        }
    }

    /// Start the IDE server over all workspace roots and consume its commands
    /// (openFile) on the UI thread.
    fn start_ide_server(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = async_channel::unbounded();
        match IdeServer::start(self.active_roots(), tx) {
            Ok(server) => self.ide = Some(server),
            Err(error) => {
                log::error!("IDE server failed to start: {error}");
                return;
            }
        }
        self._ide_task = Some(cx.spawn(async move |view, cx| {
            while let Ok(command) = rx.recv().await {
                if view
                    .update(cx, |app, cx| app.handle_ide(command, cx))
                    .is_err()
                {
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
        self.ide
            .as_ref()
            .map(|server| server.env())
            .unwrap_or_default()
    }

    fn active_roots(&self) -> Vec<PathBuf> {
        self.registry
            .workspaces
            .iter()
            .map(|workspace| workspace.root.clone())
            .collect()
    }

    fn update_ide_roots(&mut self) {
        let roots = self.active_roots();
        if let Some(server) = &mut self.ide
            && let Err(error) = server.update_roots(roots)
        {
            log::error!("failed to update IDE workspace roots: {error}");
        }
    }

    pub(crate) fn registry(&self) -> &Registry {
        &self.registry
    }

    pub(crate) fn active_stream(&self) -> Option<StreamId> {
        self.active
    }

    pub(crate) fn stream_name(&self, id: StreamId) -> &str {
        self.streams
            .get(&id)
            .map(|s| s.name.as_str())
            .unwrap_or("stream")
    }

    fn first_stream(&self) -> Option<StreamId> {
        self.registry
            .workspaces
            .iter()
            .find_map(|w| w.streams.first().copied())
    }

    fn workspace_of(&self, stream: StreamId) -> Option<&WorkspaceRec> {
        self.registry
            .workspaces
            .iter()
            .find(|w| w.streams.contains(&stream))
    }

    fn stream_root(&self, stream: StreamId) -> Option<PathBuf> {
        let workspace = self.workspace_of(stream)?;
        let record = self.streams.get(&stream)?;
        Some(record.working_dir(&workspace.root))
    }

    fn persist_active(&self) {
        if let Some(stream) = self.active
            && let Some(workspace) = self.workspace_of(stream)
        {
            let mut registry = self.registry.clone();
            registry.active = Some(Active {
                workspace: workspace.id,
                stream,
            });
            save_registry(&registry, "persist_active");
        }
    }

    /// Open the settings window, or focus/close it if already open (cmd-,).
    pub(crate) fn toggle_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            match handle.is_active(cx) {
                Some(true) => {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    self.settings_window = None;
                    return;
                }
                Some(false) => {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                    return;
                }
                None => self.settings_window = None,
            }
        }
        let bounds = Bounds::centered(None, size(px(480.), px(560.)), cx);
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Settings".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                xero_terminal::observe_appearance(window, cx).detach();
                cx.new(SettingsView::new)
            },
        ) {
            Ok(handle) => self.settings_window = Some(handle),
            Err(error) => log::error!("failed to open settings window: {error}"),
        }
    }
}

fn save_registry(registry: &Registry, context: &str) {
    if let Err(error) = xero_store::save_registry(registry) {
        log::error!("{context}: failed to save registry: {error}");
    }
}

fn save_session(workspace: WorkspaceId, stream: &Stream, context: &str) {
    if let Err(error) = xero_store::save_session(workspace, stream) {
        log::error!("{context}: failed to save session {}: {error}", stream.id);
    }
}

fn delete_session(workspace: WorkspaceId, stream: StreamId, context: &str) {
    if let Err(error) = xero_store::delete_session(workspace, stream) {
        log::error!("{context}: failed to delete session {stream}: {error}");
    }
}

/// A placeholder stream for an id whose session file is missing or corrupt.
fn synthesize_stream(id: StreamId) -> Stream {
    Stream {
        id,
        name: "main".into(),
        backing: Default::default(),
        session: Default::default(),
    }
}

/// The display name for an editor tab (file name, or the path if none).
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

impl Focusable for XeroApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
