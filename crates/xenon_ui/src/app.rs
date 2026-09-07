//! `XenonApp`: the window root. Owns the workspace registry and, per workspace,
//! a live content pane tree (mixed terminal/editor tabs). Workspaces are the
//! unit of switching; each keeps its own running PTYs while open.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Pixels, Point, PromptLevel, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, WindowBounds,
    WindowHandle, div, px,
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
    AddWorkspace, CloseEditor, DecreaseFontSize, FilePalette, IncreaseFontSize, NewFile,
    NewTerminal, OpenFile, ResetFontSize, RunTask, Save, SaveAs, ToggleBrowser, ToggleSettings,
    ToggleSidebar,
};

mod attention;
mod browser;
mod browser_menu;
mod content_ops;
mod deferred;
pub(crate) mod dirty_close;
mod editors;
mod empty_hint;
mod empty_state;
mod git_dirt;
mod keyboard;
mod live;
mod lsp;
mod nav_history;
mod nav_ops;
mod navigation;
mod open_ops;
mod palette;
mod panels;
pub(crate) mod remote;
mod render;
mod services;
mod sessions;
mod settings_window;
mod split_ops;
mod tab_context;
mod tab_drop;
mod tasks;
mod terminals;
mod themes;
mod tree_keys;
mod workspaces;

pub(crate) use attention::{AttentionMap, AttentionReason, WorkspaceDot, workspace_dot};
use deferred::{DeferredUi, FocusPane, FontPane};
pub(crate) use live::{DragTab, LiveContent, LiveLeaf, LiveNode, LiveTab};
use lsp::LspState;
use nav_history::NavHistory;
use services::AppServices;

/// Open tab context menu (right-click on a tab chip).
#[derive(Clone, Debug)]
pub(crate) struct TabContextMenu {
    pub tab: TabId,
    pub position: Point<Pixels>,
    pub selected: usize,
}

/// Right-click menu on a Files tree row (or empty tree area / header).
#[derive(Clone, Debug)]
pub(crate) struct BrowserContextMenu {
    /// Target path (file or dir). When `None`, actions target the workspace root.
    pub path: Option<PathBuf>,
    pub is_dir: bool,
    pub position: Point<Pixels>,
    pub selected: usize,
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
    theme_picker: Option<Entity<crate::theme_picker::ThemePickerView>>,
    /// File tree has keyboard focus (arrows/enter route here, not editor/terminal).
    browser_focused: bool,
    // Per-root fuzzy index shared by cmd-p and cmd-click resolution, built off
    // the UI thread. `index_tasks` keeps in-flight builds alive, keyed by root so
    // a new build for the same root replaces (cancels) the previous one.
    file_indexes: HashMap<PathBuf, Arc<FileIndex>>,
    index_tasks: HashMap<PathBuf, Task<()>>,
    /// Per-workspace recently opened editor paths (workspace-relative, MRU first).
    recent_files: HashMap<WorkspaceId, Vec<PathBuf>>,
    /// Per-workspace tab back/forward (⌘[ / ⌘]); in-memory only.
    nav_history: HashMap<WorkspaceId, NavHistory>,
    /// True while applying a history step so activate does not re-record.
    nav_suppress: bool,
    /// Overlay focus restore + window-deferred palette/command work.
    deferred: DeferredUi,
    sidebar_collapsed: bool,
    /// Live sidebar width (px); persisted per-workspace.
    sidebar_width: f32,
    /// True after a width drag until flushed to the session.
    layout_dirty: bool,
    /// Workspaces section collapsed in the left panel.
    workspaces_collapsed: bool,
    /// Pinned Workspaces list height when Files is open (`None` = content-sized).
    workspaces_section_height: Option<f32>,
    file_browser: FileBrowser,
    /// Dedicated settings window (cmd-,). None when closed or not yet opened.
    settings_window: Option<WindowHandle<SettingsView>>,
    // Workspace being renamed inline, plus its editing field.
    renaming: Option<(RenameTarget, Entity<RenameView>)>,
    _rename_sub: Option<Subscription>,
    /// Right-click menu on a terminal or editor tab.
    pub(crate) tab_menu: Option<TabContextMenu>,
    /// Right-click menu on the Files tree.
    pub(crate) browser_menu: Option<BrowserContextMenu>,
    focus: FocusHandle,
    _finder_sub: Option<Subscription>,
    _task_picker_sub: Option<Subscription>,
    _workspace_picker_sub: Option<Subscription>,
    _command_palette_sub: Option<Subscription>,
    _theme_picker_sub: Option<Subscription>,
    // Per-terminal-tab attention (sidebar + tab chips). Workspace row is derived.
    attention: AttentionMap,
    _bell_subs: Vec<Subscription>,
    // Editor selection → Claude IDE `selection_changed` push.
    _selection_subs: Vec<Subscription>,
    lsp: LspState,
    services: AppServices,
}

impl XenonApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let registry = xenon_store::load_registry().unwrap_or_default();
        let settings = xenon_store::load_settings().unwrap_or_default();
        let (lsp, lsp_events) = LspState::new(settings.lsp.clone());
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
            theme_picker: None,
            browser_focused: false,
            file_indexes: HashMap::new(),
            index_tasks: HashMap::new(),
            recent_files: HashMap::new(),
            nav_history: HashMap::new(),
            nav_suppress: false,
            deferred: DeferredUi::default(),
            sidebar_collapsed: false,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            layout_dirty: false,
            workspaces_collapsed: settings.workspaces_collapsed,
            workspaces_section_height: settings.workspaces_section_height,
            file_browser: FileBrowser::with_open(settings.files_open),
            settings_window: None,
            renaming: None,
            _rename_sub: None,
            tab_menu: None,
            browser_menu: None,
            focus: cx.focus_handle(),
            _finder_sub: None,
            _task_picker_sub: None,
            _workspace_picker_sub: None,
            _command_palette_sub: None,
            _theme_picker_sub: None,
            attention: AttentionMap::default(),
            _bell_subs: Vec::new(),
            _selection_subs: Vec::new(),
            lsp,
            services: AppServices::default(),
        };
        app.load_sessions();
        app.start_ide_server(cx);
        app.start_lsp_events(lsp_events, cx);
        app.restart_git_dirt_watch(cx);
        Self::register_main_handle(cx);
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

    /// Persist position/size (and maximized/fullscreen) after move/resize settles.
    pub fn track_window_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.services.window_bounds_subscription =
            Some(cx.observe_window_bounds(window, |this, window, cx| {
                this.queue_save_window_bounds(window, cx);
            }));
    }

    fn queue_save_window_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Trailing debounce: each move/resize restarts the timer; only the last wins.
        self.services.bounds_save_generation = self.services.bounds_save_generation.wrapping_add(1);
        let token = self.services.bounds_save_generation;
        self.services.bounds_save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            this.update_in(cx, |this, window, _cx| {
                if this.services.bounds_save_generation != token {
                    return;
                }
                this.services.bounds_save_task.take();
                persist_window_geometry(window);
            })
            .ok();
        }));
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
            Ok(server) => self.services.ide = Some(server),
            Err(error) => {
                log::error!("IDE server failed to start: {error}");
                return;
            }
        }
        self.services.ide_task = Some(cx.spawn(async move |view, cx| {
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
        self.services
            .ide
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
        if let Some(server) = &mut self.services.ide
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

    fn persist_active(&mut self) {
        if let Some(workspace) = self.active {
            if let Some(rec) = self.registry.workspace_mut(workspace) {
                rec.touch_opened();
            }
            self.registry.active = Some(Active { workspace });
            save_registry(&self.registry, "persist_active");
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

/// Write current main-window bounds into settings.json (no-op if unchanged).
fn persist_window_geometry(window: &Window) {
    let Some(geo) = geometry_from_window(window) else {
        return;
    };
    if let Err(error) = xenon_store::update_settings(|settings| {
        settings.window = Some(geo);
    }) {
        log::error!("persist window geometry failed: {error}");
    }
}

fn geometry_from_window(window: &Window) -> Option<xenon_store::WindowGeometry> {
    use xenon_store::{WindowGeometry, WindowState};
    let (bounds, state) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) => (bounds, WindowState::Windowed),
        WindowBounds::Maximized(bounds) => (bounds, WindowState::Maximized),
        WindowBounds::Fullscreen(bounds) => (bounds, WindowState::Fullscreen),
    };
    let geo = WindowGeometry::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
        state,
    );
    geo.is_sane().then_some(geo)
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
