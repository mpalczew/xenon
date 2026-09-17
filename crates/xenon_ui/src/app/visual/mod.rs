//! Visual-test scenes: drive the shipped `XenonApp` render into known states.

use gpui::{Context, Window, WindowHandle};
use xenon_store::ThemeMode;

use super::XenonApp;
use crate::settings::SettingsView;

mod chrome;
mod content;
mod overlays;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    EmptyNoWorkspaceDark,
    EmptyNoWorkspaceLight,
    EmptyNoWorkspaceTrueBlack,
    EmptyWithWorkspaceDark,
    EmptyWithWorkspaceLight,
    EmptyWithWorkspaceTrueBlack,
    PopulatedDark,
    PopulatedLight,
    PopulatedTrueBlack,
    SidebarAttention,
    SidebarWorking,
    FilesSelected,
    FilesGitDirt,
    TabsDirty,
    TabsAttention,
    TabsOverflow,
    TabsOverflowMenu,
    SplitRight,
    SplitDown,
    ReservedEmptyPane,
    FocusRing,
    TerminalGrid,
    EditorHighlight,
    MarkdownPreview,
    ImageViewer,
    Unsupported,
    EditorFind,
    TerminalFind,
    Finder,
    CommandPalette,
    KeyboardHelp,
    TaskPicker,
    WorkspacePicker,
    WorkspaceCreate,
    ThemeGallery,
    TabMenu,
    BrowserMenu,
    WorkspaceMenu,
    TerminalMenu,
    EditorMenu,
    Rename,
    Memory,
    SettingsDropdown,
    TabTooltip,
    SettingsWindow,
}

pub const SCENES: &[Scene] = &[
    Scene::EmptyNoWorkspaceDark,
    Scene::EmptyNoWorkspaceLight,
    Scene::EmptyNoWorkspaceTrueBlack,
    Scene::EmptyWithWorkspaceDark,
    Scene::EmptyWithWorkspaceLight,
    Scene::EmptyWithWorkspaceTrueBlack,
    Scene::PopulatedDark,
    Scene::PopulatedLight,
    Scene::PopulatedTrueBlack,
    Scene::SidebarAttention,
    Scene::SidebarWorking,
    Scene::FilesSelected,
    Scene::FilesGitDirt,
    Scene::TabsDirty,
    Scene::TabsAttention,
    Scene::TabsOverflow,
    Scene::TabsOverflowMenu,
    Scene::SplitRight,
    Scene::SplitDown,
    Scene::ReservedEmptyPane,
    Scene::FocusRing,
    Scene::TerminalGrid,
    Scene::EditorHighlight,
    Scene::MarkdownPreview,
    Scene::ImageViewer,
    Scene::Unsupported,
    Scene::EditorFind,
    Scene::TerminalFind,
    Scene::Finder,
    Scene::CommandPalette,
    Scene::KeyboardHelp,
    Scene::TaskPicker,
    Scene::WorkspacePicker,
    Scene::WorkspaceCreate,
    Scene::ThemeGallery,
    Scene::TabMenu,
    Scene::BrowserMenu,
    Scene::WorkspaceMenu,
    Scene::TerminalMenu,
    Scene::EditorMenu,
    Scene::Rename,
    Scene::Memory,
    Scene::SettingsDropdown,
    Scene::TabTooltip,
    Scene::SettingsWindow,
];

const REMOTE_SURFACES: &[&str] = &["remote_auth", "remote_session"];

pub fn surface_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SCENES.iter().map(|scene| scene.name()).collect();
    names.extend_from_slice(REMOTE_SURFACES);
    names
}

impl Scene {
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyNoWorkspaceDark => "chrome_empty_no_workspace_dark",
            Self::EmptyNoWorkspaceLight => "chrome_empty_no_workspace_light",
            Self::EmptyNoWorkspaceTrueBlack => "chrome_empty_no_workspace_true_black",
            Self::EmptyWithWorkspaceDark => "chrome_empty_with_workspace_dark",
            Self::EmptyWithWorkspaceLight => "chrome_empty_with_workspace_light",
            Self::EmptyWithWorkspaceTrueBlack => "chrome_empty_with_workspace_true_black",
            Self::PopulatedDark => "chrome_populated_dark",
            Self::PopulatedLight => "chrome_populated_light",
            Self::PopulatedTrueBlack => "chrome_populated_true_black",
            Self::SidebarAttention => "chrome_sidebar_attention",
            Self::SidebarWorking => "chrome_sidebar_working",
            Self::FilesSelected => "chrome_files_selected",
            Self::FilesGitDirt => "chrome_files_git_dirt",
            Self::TabsDirty => "chrome_tabs_dirty",
            Self::TabsAttention => "chrome_tabs_attention",
            Self::TabsOverflow => "chrome_tabs_overflow",
            Self::TabsOverflowMenu => "overlay_tab_overflow",
            Self::SplitRight => "chrome_split_right",
            Self::SplitDown => "chrome_split_down",
            Self::ReservedEmptyPane => "chrome_reserved_empty_pane",
            Self::FocusRing => "chrome_focus_ring",
            Self::TerminalGrid => "content_terminal_grid",
            Self::EditorHighlight => "content_editor_highlight",
            Self::MarkdownPreview => "content_markdown_preview",
            Self::ImageViewer => "content_image_viewer",
            Self::Unsupported => "content_unsupported",
            Self::EditorFind => "content_editor_find",
            Self::TerminalFind => "content_terminal_find",
            Self::Finder => "overlay_finder",
            Self::CommandPalette => "overlay_command_palette",
            Self::KeyboardHelp => "overlay_keyboard_help",
            Self::TaskPicker => "overlay_task_picker",
            Self::WorkspacePicker => "overlay_workspace_picker",
            Self::WorkspaceCreate => "overlay_workspace_create",
            Self::ThemeGallery => "overlay_theme_gallery",
            Self::TabMenu => "overlay_tab_menu",
            Self::BrowserMenu => "overlay_browser_menu",
            Self::WorkspaceMenu => "overlay_workspace_menu",
            Self::TerminalMenu => "overlay_terminal_menu",
            Self::EditorMenu => "overlay_editor_menu",
            Self::Rename => "overlay_rename",
            Self::Memory => "overlay_memory",
            Self::SettingsDropdown => "overlay_settings_dropdown",
            Self::TabTooltip => "overlay_tab_tooltip",
            Self::SettingsWindow => "settings_window",
        }
    }

    pub fn needs_terminal(self) -> bool {
        matches!(
            self,
            Self::TerminalGrid
                | Self::TerminalFind
                | Self::TerminalMenu
                | Self::TabsAttention
                | Self::TabsOverflow
                | Self::TabsOverflowMenu
        )
    }
}

pub fn apply_scene(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    match scene {
        Scene::EmptyNoWorkspaceDark
        | Scene::EmptyNoWorkspaceLight
        | Scene::EmptyNoWorkspaceTrueBlack => chrome::empty_no_workspace(app, scene, cx),
        Scene::EmptyWithWorkspaceDark
        | Scene::EmptyWithWorkspaceLight
        | Scene::EmptyWithWorkspaceTrueBlack => chrome::empty_with_workspace(app, scene, cx),
        Scene::PopulatedDark | Scene::PopulatedLight | Scene::PopulatedTrueBlack => {
            chrome::populated(app, scene, window, cx);
        }
        Scene::SidebarAttention => chrome::sidebar_attention(app, window, cx),
        Scene::SidebarWorking => chrome::sidebar_working(app, window, cx),
        Scene::FilesSelected => chrome::files_selected(app, window, cx),
        Scene::FilesGitDirt => chrome::files_git_dirt(app, window, cx),
        Scene::TabsDirty => chrome::tabs_dirty(app, window, cx),
        Scene::TabsAttention => chrome::tabs_attention(app, window, cx),
        Scene::TabsOverflow => chrome::tabs_overflow(app, window, cx, false),
        Scene::TabsOverflowMenu => chrome::tabs_overflow(app, window, cx, true),
        Scene::SplitRight => chrome::split_right(app, window, cx),
        Scene::SplitDown => chrome::split_down(app, window, cx),
        Scene::ReservedEmptyPane => chrome::reserved_empty(app, window, cx),
        Scene::FocusRing => chrome::focus_ring(app, window, cx),
        Scene::TerminalGrid => content::terminal_grid(app, window, cx),
        Scene::EditorHighlight => content::editor_highlight(app, window, cx),
        Scene::MarkdownPreview => content::markdown_preview(app, window, cx),
        Scene::ImageViewer => content::image_viewer(app, window, cx),
        Scene::Unsupported => content::unsupported(app, window, cx),
        Scene::EditorFind => content::editor_find(app, window, cx),
        Scene::TerminalFind => content::terminal_find(app, window, cx),
        Scene::Finder => overlays::finder(app, window, cx),
        Scene::CommandPalette => overlays::command_palette(app, window, cx),
        Scene::KeyboardHelp => overlays::keyboard_help(app, window, cx),
        Scene::TaskPicker => overlays::task_picker(app, window, cx),
        Scene::WorkspacePicker => overlays::workspace_picker(app, window, cx),
        Scene::WorkspaceCreate => overlays::workspace_create(app, window, cx),
        Scene::ThemeGallery => overlays::theme_gallery(app, window, cx),
        Scene::TabMenu => overlays::tab_menu(app, window, cx),
        Scene::BrowserMenu => overlays::browser_menu(app, window, cx),
        Scene::WorkspaceMenu => overlays::workspace_menu(app, window, cx),
        Scene::TerminalMenu => overlays::terminal_menu(app, window, cx),
        Scene::EditorMenu => overlays::editor_menu(app, window, cx),
        Scene::Rename => overlays::rename(app, window, cx),
        Scene::Memory => overlays::memory(app, window, cx),
        Scene::SettingsDropdown => overlays::settings_dropdown(app, window, cx),
        Scene::TabTooltip => overlays::tab_tooltip(app, window, cx),
        Scene::SettingsWindow => overlays::settings_window(app, cx),
    }
}

impl XenonApp {
    pub fn visual_settings_window(&self) -> Option<WindowHandle<SettingsView>> {
        self.settings_window
    }

    pub fn visual_terminal_ready(&self, cx: &gpui::App) -> bool {
        self.active_terminal()
            .is_some_and(|view| view.read(cx).visual_is_ready())
    }
}

fn set_theme(mode: ThemeMode, dark: &str, light: &str, cx: &mut Context<XenonApp>) {
    let mut settings = xenon_settings::snapshot(cx);
    settings.theme = mode;
    settings.dark_theme = dark.to_string();
    settings.light_theme = light.to_string();
    xenon_settings::apply(&settings, cx);
    xenon_terminal::apply_theme(cx);
    cx.notify();
}

fn theme_for(scene: Scene, cx: &mut Context<XenonApp>) {
    match scene {
        Scene::EmptyNoWorkspaceLight | Scene::EmptyWithWorkspaceLight | Scene::PopulatedLight => {
            set_theme(ThemeMode::Light, "One Dark", "One Light", cx);
        }
        Scene::EmptyNoWorkspaceTrueBlack
        | Scene::EmptyWithWorkspaceTrueBlack
        | Scene::PopulatedTrueBlack => {
            set_theme(ThemeMode::Dark, "True Black", "One Light", cx);
        }
        _ => set_theme(ThemeMode::Dark, "One Dark", "One Light", cx),
    }
}

fn workspace_root(app: &XenonApp) -> Option<std::path::PathBuf> {
    app.active.and_then(|id| {
        app.registry
            .workspace(id)
            .map(|workspace| workspace.root.clone())
    })
}

fn open_rel(app: &mut XenonApp, rel: &str, window: &mut Window, cx: &mut Context<XenonApp>) {
    let Some(root) = workspace_root(app) else {
        return;
    };
    let _ = window;
    app.open_editor(root.join(rel), true, cx);
}

fn populate(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    open_rel(app, "src/main.rs", window, cx);
    open_rel(app, "NOTES.md", window, cx);
}

fn ensure_terminal(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    if app.active_terminal().is_none() {
        app.new_terminal(window, cx);
    }
}
