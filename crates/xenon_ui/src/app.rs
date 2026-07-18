//! `XenonApp`: the window root. Owns the workspace registry and, per workspace,
//! a live content pane tree (mixed terminal/editor tabs). Workspaces are the
//! unit of switching; each keeps its own running PTYs while open.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Pixels, Point, PromptLevel, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, WindowHandle,
    div, px,
};
use theme::ActiveTheme;
use xenon_core::{
    Active, DEFAULT_SIDEBAR_WIDTH, Registry, SessionState, TabId, WorkspaceId, WorkspaceRec,
};
use xenon_editor::{EditorEvent, EditorView};
use xenon_finder::{FileIndex, Finder};
use xenon_ide::{IdeCommand, IdeServer, SelectionSnapshot};
use xenon_terminal::{TerminalEvent, TerminalView};

use crate::file_browser::{FileBrowser, TreeRow, dir_marker, file_icon};
use crate::finder::{FinderEvent, FinderView};
use crate::rename::{RenameEvent, RenameView};
use crate::settings::SettingsView;
use crate::task_picker::TaskPickerView;
use crate::workspace_picker::WorkspacePickerView;
use crate::{
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, NewTerminal,
    OpenFile, ResetFontSize, RunTask, Save, ToggleBrowser, ToggleSettings, ToggleSidebar,
};

mod browser;
mod content_ops;
mod deferred;
pub(crate) mod dirty_close;
mod editors;
mod empty_hint;
mod git_dirt;
mod keyboard;
mod live;
mod navigation;
mod palette;
mod panels;
mod render;
mod sessions;
mod settings_window;
mod split_ops;
mod tab_drop;
mod tasks;
mod terminals;
mod tree_keys;
mod workspaces;

use deferred::{DeferredUi, FocusPane, FontPane};
pub(crate) use live::{DragTab, LiveContent, LiveLeaf, LiveNode, LiveTab};
use sessions::AttentionReason;

/// Open tab context menu (close only).
#[derive(Clone, Debug)]
pub(crate) struct TabContextMenu {
    pub tab: TabId,
    pub position: Point<Pixels>,
}

/// What the sidebar inline rename field is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameTarget {
    Workspace(WorkspaceId),
}

pub struct XenonApp {
    registry: Registry,
    /// Per-workspace session layout metadata (persisted).
    sessions: HashMap<WorkspaceId, SessionState>,
    /// Per workspace: live content pane tree.
    pub(crate) contents: HashMap<WorkspaceId, LiveContent>,
    pub(crate) active: Option<WorkspaceId>,
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
    /// Per-workspace recently opened editor paths (workspace-relative, MRU first).
    recent_files: HashMap<WorkspaceId, Vec<PathBuf>>,
    /// Overlay focus restore + window-deferred palette/command work.
    deferred: DeferredUi,
    sidebar_collapsed: bool,
    /// Live sidebar width (px); persisted per-workspace.
    sidebar_width: f32,
    /// True after a width drag until flushed to the session.
    layout_dirty: bool,
    /// Workspaces section collapsed in the left panel.
    workspaces_collapsed: bool,
    file_browser: FileBrowser,
    /// Dedicated settings window (cmd-,). None when closed or not yet opened.
    settings_window: Option<WindowHandle<SettingsView>>,
    // Workspace being renamed inline, plus its editing field.
    renaming: Option<(RenameTarget, Entity<RenameView>)>,
    _rename_sub: Option<Subscription>,
    /// Right-click menu on a terminal or editor tab.
    pub(crate) tab_menu: Option<TabContextMenu>,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
    _task_picker_sub: Option<Subscription>,
    _workspace_picker_sub: Option<Subscription>,
    _command_palette_sub: Option<Subscription>,
    // Workspaces flagged for attention (sidebar dot). Value is the last reason,
    // shown on hover for debugging. Cleared when the workspace is selected.
    attention: HashMap<WorkspaceId, AttentionReason>,
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

impl XenonApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let registry = xenon_store::load_registry().unwrap_or_default();
        let settings = xenon_store::load_settings().unwrap_or_default();
        xenon_settings::apply(&settings, cx);
        xenon_terminal::apply_theme(cx);
        let mut app = Self {
            registry,
            sessions: HashMap::new(),
            contents: HashMap::new(),
            active: None,
            finder: None,
            task_picker: None,
            workspace_picker: None,
            command_palette: None,
            browser_focused: false,
            file_indexes: HashMap::new(),
            index_tasks: HashMap::new(),
            recent_files: HashMap::new(),
            deferred: DeferredUi::default(),
            sidebar_collapsed: false,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            layout_dirty: false,
            workspaces_collapsed: settings.workspaces_collapsed,
            file_browser: FileBrowser::with_open(settings.files_open),
            settings_window: None,
            renaming: None,
            _rename_sub: None,
            tab_menu: None,
            focus: cx.focus_handle(),
            _finder_sub: None,
            _task_picker_sub: None,
            _workspace_picker_sub: None,
            _command_palette_sub: None,
            attention: HashMap::new(),
            _bell_subs: Vec::new(),
            _selection_subs: Vec::new(),
            ide: None,
            _ide_task: None,
            git_dirt: HashMap::new(),
            git_dirt_tx: None,
            _git_dirt_task: None,
        };
        app.load_sessions();
        app.start_ide_server(cx);
        app.restart_git_dirt_watch(cx);
        let active = app
            .registry
            .active
            .map(|a| a.workspace)
            .filter(|id| app.registry.workspace(*id).is_some())
            .or_else(|| app.first_workspace());
        if let Some(id) = active {
            app.activate_workspace(id, cx);
        }
        app
    }

    /// Load session metadata for every open workspace (defaults when missing).
    fn load_sessions(&mut self) {
        for workspace in &self.registry.workspaces {
            let session = xenon_store::load_session(workspace.id).unwrap_or_default();
            self.sessions.insert(workspace.id, session);
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

    pub(crate) fn active_workspace(&self) -> Option<WorkspaceId> {
        self.active
    }

    fn first_workspace(&self) -> Option<WorkspaceId> {
        self.registry.workspaces.first().map(|w| w.id)
    }

    fn workspace_root(&self, id: WorkspaceId) -> Option<PathBuf> {
        self.registry.workspace(id).map(|w| w.root.clone())
    }

    fn persist_active(&self) {
        if let Some(workspace) = self.active {
            let mut registry = self.registry.clone();
            registry.active = Some(Active { workspace });
            save_registry(&registry, "persist_active");
        }
    }
}

fn save_registry(registry: &Registry, context: &str) {
    if let Err(error) = xenon_store::save_registry(registry) {
        log::error!("{context}: failed to save registry: {error}");
    }
}

fn save_session(workspace: WorkspaceId, session: &SessionState, context: &str) {
    if let Err(error) = xenon_store::save_session(workspace, session) {
        log::error!("{context}: failed to save session {workspace}: {error}");
    }
}

/// Persist Workspaces/Files section collapse into settings.json.
fn persist_section_prefs(workspaces_collapsed: bool, files_open: bool) {
    let mut settings = xenon_store::load_settings().unwrap_or_default();
    if settings.workspaces_collapsed == workspaces_collapsed && settings.files_open == files_open {
        return;
    }
    settings.workspaces_collapsed = workspaces_collapsed;
    settings.files_open = files_open;
    if let Err(error) = xenon_store::save_settings(&settings) {
        log::error!("persist section prefs failed: {error}");
    }
}

/// The display name for an editor tab (file name, or the path if none).
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

impl Focusable for XenonApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
