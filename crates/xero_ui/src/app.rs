//! `XeroApp`: the window root. Owns the workspace registry and, per stream, a
//! live terminal + editor. Streams are the unit of switching: each workspace
//! holds one or more, and every stream keeps its own running PTY.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Pixels, Point, PromptLevel, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, WindowHandle,
    div, px,
};
use theme::ActiveTheme;
use xero_core::{Active, Layout, Registry, Stream, StreamId, WorkspaceId, WorkspaceRec};
use xero_editor::{EditorEvent, EditorView};
use xero_finder::{FileIndex, Finder};
use xero_ide::{IdeCommand, IdeServer, SelectionSnapshot};
use xero_terminal::{TerminalEvent, TerminalView};

use crate::file_browser::{FileBrowser, TreeRow, dir_marker, file_icon};
use crate::finder::{FinderEvent, FinderView};
use crate::rename::{RenameEvent, RenameView};
use crate::settings::SettingsView;
use crate::task_picker::TaskPickerView;
use crate::workspace_picker::WorkspacePickerView;
use crate::{
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, NewStream,
    NewTerminal, OpenFile, ResetFontSize, ToggleBrowser, ToggleEditor, ToggleSettings,
    ToggleSidebar, ToggleTerminal,
};

mod browser;
mod dirty_close;
mod editors;
mod empty_hint;
mod git_dirt;
mod keyboard;
mod navigation;
mod palette;
mod panels;
mod render;
mod settings_window;
mod streams;
mod tasks;
mod terminals;
mod tree_keys;
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

/// Right-click target on a terminal or editor tab chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabSurface {
    Terminal,
    Editor,
}

/// Open move-tab context menu (stream + index captured at open time).
#[derive(Clone, Debug)]
pub(crate) struct TabContextMenu {
    pub surface: TabSurface,
    pub stream: StreamId,
    pub index: usize,
    pub position: Point<Pixels>,
    /// Keyboard highlight into menu rows (0 = Move to New Stream).
    pub selected: usize,
}

/// Source tab + destination stream for a move.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TabMove {
    pub from: StreamId,
    pub index: usize,
    pub to: StreamId,
}

/// What the sidebar inline rename field is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameTarget {
    Stream(StreamId),
    Workspace(WorkspaceId),
}

/// Which pane held keyboard focus before the finder opened, so Escape can
/// return focus there instead of dropping it into the void.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Terminal,
    Editor,
    Browser,
}

/// Content surface for cmd-+ / cmd-- font zoom (editor and terminal only).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FontPane {
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
    task_picker: Option<Entity<TaskPickerView>>,
    workspace_picker: Option<Entity<WorkspacePickerView>>,
    command_palette: Option<Entity<crate::command_palette::CommandPaletteView>>,
    /// File tree has keyboard focus (arrows/enter route here, not editor/terminal).
    browser_focused: bool,
    // Per-root fuzzy index shared by cmd-p and cmd-click resolution, built off
    // the UI thread. `index_tasks` keeps in-flight builds alive, keyed by root so
    // a new build for the same root replaces (cancels) the previous one.
    file_indexes: HashMap<PathBuf, Arc<FileIndex>>,
    index_tasks: HashMap<PathBuf, Task<()>>,
    // The pane focused when the finder opened, and (on dismiss) the pane to
    // re-focus during the next render (render is where a `Window` is available).
    restore_pane: Option<FocusPane>,
    pending_focus: Option<FocusPane>,
    /// Last editor/terminal focus for zoom when chrome (sidebar, tree) has focus.
    last_font_pane: FontPane,
    // A cmd-clicked name to open cmd-p with, deferred to render (which has a
    // Window) from the windowless terminal-event subscription.
    pending_palette_query: Option<String>,
    /// Command / stream jump deferred until render has a Window.
    pending_command: Option<crate::commands::CommandId>,
    pending_stream: Option<StreamId>,
    sidebar_collapsed: bool,
    terminal_collapsed: bool,
    editor_collapsed: bool,
    /// Live pane widths (px); persisted per-stream via `Layout`.
    sidebar_width: f32,
    tree_width: f32,
    terminal_width: f32,
    /// True after a width drag until flushed to the session.
    layout_dirty: bool,
    // Workspaces whose streams are hidden in the sidebar.
    collapsed_workspaces: HashSet<WorkspaceId>,
    // Closed-workspace list is collapsed by default (archive, not peer list).
    closed_section_collapsed: bool,
    file_browser: FileBrowser,
    /// Dedicated settings window (cmd-,). None when closed or not yet opened.
    settings_window: Option<WindowHandle<SettingsView>>,
    // Stream or workspace being renamed inline, plus its editing field.
    renaming: Option<(RenameTarget, Entity<RenameView>)>,
    _rename_sub: Option<Subscription>,
    /// Right-click menu on a terminal or editor tab (move to stream).
    pub(crate) tab_menu: Option<TabContextMenu>,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
    _task_picker_sub: Option<Subscription>,
    _workspace_picker_sub: Option<Subscription>,
    _command_palette_sub: Option<Subscription>,
    // Streams whose terminal rang the bell while unfocused (agent wants
    // attention); shown as a dot in the sidebar, cleared when the stream opens.
    attention: HashSet<StreamId>,
    _bell_subs: Vec<Subscription>,
    // Editor selection → Claude IDE `selection_changed` push.
    _selection_subs: Vec<Subscription>,
    // IDE server: agents in the terminal connect here to drive xero. Its env is
    // injected into every terminal so Claude Code discovers it.
    ide: Option<IdeServer>,
    _ide_task: Option<Task<()>>,
    // Live git dirt totals per workspace (dirty only); refreshed by FS events.
    git_dirt: HashMap<WorkspaceId, crate::git_dirt::GitDirt>,
    /// Commands into the long-lived git-dirt task (root set changes).
    git_dirt_tx: Option<async_channel::Sender<git_dirt::DirtMsg>>,
    _git_dirt_task: Option<Task<()>>,
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
            task_picker: None,
            workspace_picker: None,
            command_palette: None,
            browser_focused: false,
            file_indexes: HashMap::new(),
            index_tasks: HashMap::new(),
            restore_pane: None,
            pending_focus: None,
            last_font_pane: FontPane::Terminal,
            pending_palette_query: None,
            pending_command: None,
            pending_stream: None,
            sidebar_collapsed: false,
            terminal_collapsed: false,
            editor_collapsed: false,
            sidebar_width: Layout::default().sidebar_width,
            tree_width: Layout::default().tree_width,
            terminal_width: Layout::default().terminal_width,
            layout_dirty: false,
            collapsed_workspaces: HashSet::new(),
            closed_section_collapsed: true,
            file_browser: FileBrowser::default(),
            settings_window: None,
            renaming: None,
            _rename_sub: None,
            tab_menu: None,
            focus: cx.focus_handle(),
            _finder_sub: None,
            _task_picker_sub: None,
            _workspace_picker_sub: None,
            _command_palette_sub: None,
            attention: HashSet::new(),
            _bell_subs: Vec::new(),
            _selection_subs: Vec::new(),
            ide: None,
            _ide_task: None,
            git_dirt: HashMap::new(),
            git_dirt_tx: None,
            _git_dirt_task: None,
        };
        app.load_streams();
        app.start_ide_server(cx);
        app.start_git_dirt_watch(cx);
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
