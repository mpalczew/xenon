//! Visual-test scenes: drive the shipped `XenonApp` render into known states.

use gpui::{Context, Window};
use xenon_store::ThemeMode;

use super::XenonApp;

mod chrome;
mod content;
mod git;
mod hooks;
mod keyboard;
mod overlays;
mod remote;
mod settings;
mod ssh;
mod ssh_picker;
mod worklist;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    EmptyNoWorkspaceDark,
    EmptyWithWorkspaceDark,
    PopulatedDark,
    PopulatedLight,
    SidebarAttention,
    SidebarWorking,
    FilesSelected,
    SidebarMarkers,
    SidebarMarkersLight,
    GitExpanded,
    GitExpandedLight,
    GitCollapsed,
    GitClean,
    GitKeyboard,
    DiffSplit,
    DiffSplitLight,
    DiffUnified,
    DiffNarrowAuto,
    DiffIndexOnly,
    TabsDirty,
    TabsAttention,
    TabsOverflow,
    TabsOverflowMenu,
    SplitRight,
    SplitRow,
    SplitMixed,
    ReservedEmptyPane,
    FocusRing,
    TerminalGrid,
    EditorHighlight,
    EditorWrap,
    MarkdownPreview,
    MarkdownPreviewSplit,
    ImageViewer,
    Unsupported,
    EditorFind,
    TerminalFind,
    Finder,
    CommandPalette,
    KeyboardHelp,
    TaskPicker,
    WorkspacePicker,
    SshWorkspacePicker,
    SshHostPicker,
    SshHostResults,
    SshHostSearching,
    SshHostError,
    SshSchemeSuggest,
    SshWorkspace,
    SshWorkspaceUnavailable,
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
    SkillPromptDark,
    SettingsAgentsInstalled,
    WorklistCapture,
    WorklistCaptureLong,
    WorklistTab,
    WorklistItemEdit,
    WorklistMarkdown,
    WorklistEmpty,
    WorklistInlineCapture,
    WorklistSections,
    WorklistSectionsFolded,
    WorklistSectionRename,
    WorklistToolbarTooltip,
    ToastSuccess,
    ToastError,
    ToastLongKeys,
    ConnectPhone,
    ConnectPhoneOffline,
    PhoneDriving,
    SettingsRemote,
    SettingsEditor,
    SettingsLanguageServers,
    SettingsSearch,
    ThemePanelPreview,
}

pub const SCENES: &[Scene] = &[
    Scene::EmptyNoWorkspaceDark,
    Scene::EmptyWithWorkspaceDark,
    Scene::PopulatedDark,
    Scene::PopulatedLight,
    Scene::SidebarAttention,
    Scene::SidebarWorking,
    Scene::FilesSelected,
    Scene::SidebarMarkers,
    Scene::SidebarMarkersLight,
    Scene::GitExpanded,
    Scene::GitExpandedLight,
    Scene::GitCollapsed,
    Scene::GitClean,
    Scene::GitKeyboard,
    Scene::DiffSplit,
    Scene::DiffSplitLight,
    Scene::DiffUnified,
    Scene::DiffNarrowAuto,
    Scene::DiffIndexOnly,
    Scene::TabsDirty,
    Scene::TabsAttention,
    Scene::TabsOverflow,
    Scene::TabsOverflowMenu,
    Scene::SplitRight,
    Scene::SplitRow,
    Scene::SplitMixed,
    Scene::ReservedEmptyPane,
    Scene::FocusRing,
    Scene::TerminalGrid,
    Scene::EditorHighlight,
    Scene::EditorWrap,
    Scene::MarkdownPreview,
    Scene::MarkdownPreviewSplit,
    Scene::ImageViewer,
    Scene::Unsupported,
    Scene::EditorFind,
    Scene::TerminalFind,
    Scene::Finder,
    Scene::CommandPalette,
    Scene::KeyboardHelp,
    Scene::TaskPicker,
    Scene::WorkspacePicker,
    Scene::SshWorkspacePicker,
    Scene::SshHostPicker,
    Scene::SshHostResults,
    Scene::SshHostSearching,
    Scene::SshHostError,
    Scene::SshSchemeSuggest,
    Scene::SshWorkspace,
    Scene::SshWorkspaceUnavailable,
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
    Scene::SkillPromptDark,
    Scene::SettingsAgentsInstalled,
    Scene::WorklistCapture,
    Scene::WorklistCaptureLong,
    Scene::WorklistTab,
    Scene::WorklistItemEdit,
    Scene::WorklistMarkdown,
    Scene::WorklistEmpty,
    Scene::WorklistInlineCapture,
    Scene::WorklistSections,
    Scene::WorklistSectionsFolded,
    Scene::WorklistSectionRename,
    Scene::WorklistToolbarTooltip,
    Scene::ToastSuccess,
    Scene::ToastError,
    Scene::ToastLongKeys,
    Scene::ConnectPhone,
    Scene::ConnectPhoneOffline,
    Scene::PhoneDriving,
    Scene::SettingsRemote,
    Scene::SettingsEditor,
    Scene::SettingsLanguageServers,
    Scene::SettingsSearch,
    Scene::ThemePanelPreview,
];

impl Scene {
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyNoWorkspaceDark => "chrome_empty_no_workspace_dark",
            Self::EmptyWithWorkspaceDark => "chrome_empty_with_workspace_dark",
            Self::PopulatedDark => "chrome_populated_dark",
            Self::PopulatedLight => "chrome_populated_light",
            Self::SidebarAttention => "chrome_sidebar_attention",
            Self::SidebarWorking => "chrome_sidebar_working",
            Self::FilesSelected => "chrome_files_selected",
            Self::SidebarMarkers => "chrome_sidebar_markers",
            Self::SidebarMarkersLight => "chrome_sidebar_markers_light",
            Self::GitExpanded => "chrome_git_expanded",
            Self::GitExpandedLight => "chrome_git_expanded_light",
            Self::GitCollapsed => "chrome_git_collapsed",
            Self::GitClean => "chrome_git_clean",
            Self::GitKeyboard => "chrome_git_keyboard",
            Self::DiffSplit => "content_diff_split",
            Self::DiffSplitLight => "content_diff_split_light",
            Self::DiffUnified => "content_diff_unified",
            Self::DiffNarrowAuto => "content_diff_narrow_auto",
            Self::DiffIndexOnly => "content_diff_index_only",
            Self::TabsDirty => "chrome_tabs_dirty",
            Self::TabsAttention => "chrome_tabs_attention",
            Self::TabsOverflow => "chrome_tabs_overflow",
            Self::TabsOverflowMenu => "overlay_tab_overflow",
            Self::SplitRight => "chrome_split_right",
            Self::SplitRow => "chrome_split_row",
            Self::SplitMixed => "chrome_split_mixed",
            Self::ReservedEmptyPane => "chrome_reserved_empty_pane",
            Self::FocusRing => "chrome_focus_ring",
            Self::TerminalGrid => "content_terminal_grid",
            Self::EditorHighlight => "content_editor_highlight",
            Self::EditorWrap => "content_editor_wrap",
            Self::MarkdownPreview => "content_markdown_preview",
            Self::MarkdownPreviewSplit => "content_markdown_preview_split",
            Self::ImageViewer => "content_image_viewer",
            Self::Unsupported => "content_unsupported",
            Self::EditorFind => "content_editor_find",
            Self::TerminalFind => "content_terminal_find",
            Self::Finder => "overlay_finder",
            Self::CommandPalette => "overlay_command_palette",
            Self::KeyboardHelp => "overlay_keyboard_help",
            Self::TaskPicker => "overlay_task_picker",
            Self::WorkspacePicker => "overlay_workspace_picker",
            Self::SshWorkspacePicker => "overlay_ssh_workspace_picker",
            Self::SshHostPicker => "overlay_ssh_host_picker",
            Self::SshHostResults => "overlay_ssh_host_results",
            Self::SshHostSearching => "overlay_ssh_host_searching",
            Self::SshHostError => "overlay_ssh_host_error",
            Self::SshSchemeSuggest => "overlay_ssh_scheme_suggest",
            Self::SshWorkspace => "chrome_ssh_workspace",
            Self::SshWorkspaceUnavailable => "chrome_ssh_workspace_unavailable",
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
            Self::SkillPromptDark => "overlay_skill_prompt_dark",
            Self::SettingsAgentsInstalled => "settings_agents_installed",
            Self::WorklistCapture => "overlay_worklist_capture",
            Self::WorklistCaptureLong => "overlay_worklist_capture_long",
            Self::WorklistTab => "content_worklist_tab",
            Self::WorklistItemEdit => "content_worklist_item_edit",
            Self::WorklistMarkdown => "content_worklist_markdown",
            Self::WorklistEmpty => "content_worklist_empty",
            Self::WorklistInlineCapture => "content_worklist_inline_capture",
            Self::WorklistSections => "content_worklist_sections",
            Self::WorklistSectionsFolded => "content_worklist_sections_folded",
            Self::WorklistSectionRename => "content_worklist_section_rename",
            Self::WorklistToolbarTooltip => "overlay_worklist_toolbar_tooltip",
            Self::ToastSuccess => "overlay_toast_success",
            Self::ToastError => "overlay_toast_error",
            Self::ToastLongKeys => "overlay_toast_long_keys",
            Self::ConnectPhone => "overlay_connect_phone",
            Self::ConnectPhoneOffline => "overlay_connect_phone_no_tailscale",
            Self::PhoneDriving => "content_terminal_phone_fit",
            Self::SettingsRemote => "settings_remote",
            Self::SettingsEditor => "settings_editor",
            Self::SettingsLanguageServers => "settings_language_servers",
            Self::SettingsSearch => "settings_search",
            Self::ThemePanelPreview => "overlay_theme_panel_preview",
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
                | Self::PhoneDriving
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
        Scene::EmptyNoWorkspaceDark => chrome::empty_no_workspace(app, scene, cx),
        Scene::EmptyWithWorkspaceDark => chrome::empty_with_workspace(app, scene, cx),
        Scene::PopulatedDark | Scene::PopulatedLight => {
            chrome::populated(app, scene, window, cx);
        }
        Scene::SidebarAttention => chrome::sidebar_attention(app, window, cx),
        Scene::SidebarWorking => chrome::sidebar_working(app, window, cx),
        Scene::FilesSelected => chrome::files_selected(app, window, cx),
        Scene::SidebarMarkers => git::sidebar_markers(app, false, window, cx),
        Scene::SidebarMarkersLight => git::sidebar_markers(app, true, window, cx),
        Scene::GitExpanded
        | Scene::GitExpandedLight
        | Scene::GitCollapsed
        | Scene::GitClean
        | Scene::GitKeyboard => git::section(app, scene, window, cx),
        Scene::DiffSplit
        | Scene::DiffSplitLight
        | Scene::DiffUnified
        | Scene::DiffNarrowAuto
        | Scene::DiffIndexOnly => git::diff(app, scene, window, cx),
        Scene::TabsDirty => chrome::tabs_dirty(app, window, cx),
        Scene::TabsAttention => chrome::tabs_attention(app, window, cx),
        Scene::TabsOverflow => chrome::tabs_overflow(app, window, cx, false),
        Scene::TabsOverflowMenu => chrome::tabs_overflow(app, window, cx, true),
        Scene::SplitRight => chrome::split_right(app, window, cx),
        Scene::SplitRow => chrome::split_row(app, window, cx),
        Scene::SplitMixed => chrome::split_mixed(app, window, cx),
        Scene::ReservedEmptyPane => chrome::reserved_empty(app, window, cx),
        Scene::FocusRing => chrome::focus_ring(app, window, cx),
        Scene::TerminalGrid => content::terminal_grid(app, window, cx),
        Scene::EditorHighlight => content::editor_highlight(app, window, cx),
        Scene::EditorWrap => content::editor_wrap(app, window, cx),
        Scene::MarkdownPreview => content::markdown_preview(app, window, cx),
        Scene::MarkdownPreviewSplit => content::markdown_preview_split(app, window, cx),
        Scene::ImageViewer => content::image_viewer(app, window, cx),
        Scene::Unsupported => content::unsupported(app, window, cx),
        Scene::EditorFind => content::editor_find(app, window, cx),
        Scene::TerminalFind => content::terminal_find(app, window, cx),
        Scene::Finder => overlays::finder(app, window, cx),
        Scene::CommandPalette => overlays::command_palette(app, window, cx),
        Scene::KeyboardHelp => overlays::keyboard_help(app, window, cx),
        Scene::TaskPicker => overlays::task_picker(app, window, cx),
        Scene::WorkspacePicker => overlays::workspace_picker(app, window, cx),
        Scene::SshWorkspacePicker => overlays::ssh_workspace_picker(app, window, cx),
        Scene::SshHostPicker
        | Scene::SshHostResults
        | Scene::SshHostSearching
        | Scene::SshHostError
        | Scene::SshSchemeSuggest => ssh_picker::scene(app, scene, window, cx),
        Scene::SshWorkspace => ssh::workspace(app, false, window, cx),
        Scene::SshWorkspaceUnavailable => ssh::workspace(app, true, window, cx),
        Scene::WorkspaceCreate => overlays::workspace_create(app, window, cx),
        Scene::ThemeGallery => overlays::theme_gallery(app, window, cx),
        Scene::ThemePanelPreview => overlays::theme_panel_preview(app, window, cx),
        Scene::TabMenu => overlays::tab_menu(app, window, cx),
        Scene::BrowserMenu => overlays::browser_menu(app, window, cx),
        Scene::WorkspaceMenu => overlays::workspace_menu(app, window, cx),
        Scene::TerminalMenu => overlays::terminal_menu(app, window, cx),
        Scene::EditorMenu => overlays::editor_menu(app, window, cx),
        Scene::Rename => overlays::rename(app, window, cx),
        Scene::Memory => overlays::memory(app, window, cx),
        Scene::SettingsDropdown => settings::dropdown(app, window, cx),
        Scene::TabTooltip => overlays::tab_tooltip(app, window, cx),
        Scene::SettingsWindow
        | Scene::SettingsEditor
        | Scene::SettingsLanguageServers
        | Scene::SettingsSearch => settings::page(app, scene, cx),
        Scene::SkillPromptDark => overlays::skill_prompt(app, scene, window, cx),
        Scene::SettingsAgentsInstalled => settings::agents_installed(app, cx),
        Scene::ConnectPhone => remote::connect_phone(app, true, window, cx),
        Scene::ConnectPhoneOffline => remote::connect_phone(app, false, window, cx),
        Scene::PhoneDriving => content::phone_driving(app, window, cx),
        Scene::SettingsRemote => settings::remote(app, cx),
        Scene::WorklistCapture | Scene::WorklistCaptureLong => {
            worklist::capture(app, scene, window, cx)
        }
        Scene::WorklistTab
        | Scene::WorklistItemEdit
        | Scene::WorklistMarkdown
        | Scene::WorklistEmpty
        | Scene::WorklistInlineCapture
        | Scene::WorklistSections
        | Scene::WorklistSectionsFolded
        | Scene::WorklistSectionRename => worklist::tab(app, scene, window, cx),
        Scene::WorklistToolbarTooltip => chrome::populated(app, scene, window, cx),
        Scene::ToastSuccess | Scene::ToastError | Scene::ToastLongKeys => {
            chrome::populated(app, scene, window, cx);
            let toast = match scene {
                Scene::ToastLongKeys => super::toasts::long_keys_sample(),
                Scene::ToastSuccess => {
                    super::toasts::task_added(Some("Fix focus after closing a split"))
                }
                _ => super::toasts::worklist_error(
                    "Can’t undo just yet",
                    "Save or undo the worklist Markdown edits first",
                ),
            };
            app.show_toast(toast, cx);
        }
    }
}

pub(super) fn set_theme(mode: ThemeMode, dark: &str, light: &str, cx: &mut Context<XenonApp>) {
    let mut settings = xenon_settings::snapshot(cx);
    settings.theme = mode;
    settings.dark_theme = dark.to_string();
    settings.light_theme = light.to_string();
    xenon_settings::apply(&settings, cx);
    xenon_terminal::apply_theme(cx);
    cx.notify();
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
