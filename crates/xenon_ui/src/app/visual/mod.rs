//! Visual-test scenes: drive the shipped `XenonApp` render into known states.

use gpui::{Context, Window, WindowHandle};
use xenon_store::ThemeMode;

use super::XenonApp;
use crate::settings::SettingsView;

mod chrome;
mod content;
mod keyboard;
mod overlays;
mod remote;
mod settings;
mod worklist;

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
    SettingsDropdownFiltered,
    TabTooltip,
    SettingsWindow,
    SkillPromptDark,
    SkillPromptLight,
    SettingsAgentsInstalled,
    WorklistCapture,
    WorklistCaptureFilled,
    WorklistCaptureLong,
    WorklistTab,
    WorklistItemEdit,
    WorklistMarkdown,
    WorklistEmpty,
    WorklistEmptyLight,
    WorklistEmptyVirtual,
    WorklistInlineCapture,
    WorklistInlineLight,
    WorklistSections,
    WorklistSectionsLight,
    WorklistSectionsFolded,
    WorklistSectionRename,
    WorklistCaptureSection,
    WorklistToolbarTooltip,
    ToastSuccess,
    ToastInfo,
    ToastError,
    ToastErrorLight,
    ToastKeymapError,
    ToastLongKeys,
    ConnectPhone,
    ConnectPhoneOffline,
    PhoneDriving,
    SettingsRemote,
    SettingsEditor,
    SettingsLanguageServers,
    SettingsSearch,
    SettingsLight,
    ThemePanelPreview,
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
    Scene::SettingsDropdownFiltered,
    Scene::TabTooltip,
    Scene::SettingsWindow,
    Scene::SkillPromptDark,
    Scene::SkillPromptLight,
    Scene::SettingsAgentsInstalled,
    Scene::WorklistCapture,
    Scene::WorklistCaptureFilled,
    Scene::WorklistCaptureLong,
    Scene::WorklistTab,
    Scene::WorklistItemEdit,
    Scene::WorklistMarkdown,
    Scene::WorklistEmpty,
    Scene::WorklistEmptyLight,
    Scene::WorklistEmptyVirtual,
    Scene::WorklistInlineCapture,
    Scene::WorklistInlineLight,
    Scene::WorklistSections,
    Scene::WorklistSectionsLight,
    Scene::WorklistSectionsFolded,
    Scene::WorklistSectionRename,
    Scene::WorklistCaptureSection,
    Scene::WorklistToolbarTooltip,
    Scene::ToastSuccess,
    Scene::ToastInfo,
    Scene::ToastError,
    Scene::ToastErrorLight,
    Scene::ToastKeymapError,
    Scene::ToastLongKeys,
    Scene::ConnectPhone,
    Scene::ConnectPhoneOffline,
    Scene::PhoneDriving,
    Scene::SettingsRemote,
    Scene::SettingsEditor,
    Scene::SettingsLanguageServers,
    Scene::SettingsSearch,
    Scene::SettingsLight,
    Scene::ThemePanelPreview,
];

const REMOTE_SURFACES: &[&str] = &[
    "remote_pair",
    "remote_code",
    "remote_workspaces",
    "remote_session",
    "remote_session_light",
    "remote_reconnecting",
];

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
            Self::SettingsDropdownFiltered => "overlay_settings_dropdown_filtered",
            Self::TabTooltip => "overlay_tab_tooltip",
            Self::SettingsWindow => "settings_window",
            Self::SkillPromptDark => "overlay_skill_prompt_dark",
            Self::SkillPromptLight => "overlay_skill_prompt_light",
            Self::SettingsAgentsInstalled => "settings_agents_installed",
            Self::WorklistCapture => "overlay_worklist_capture",
            Self::WorklistCaptureFilled => "overlay_worklist_capture_filled",
            Self::WorklistCaptureLong => "overlay_worklist_capture_long",
            Self::WorklistTab => "content_worklist_tab",
            Self::WorklistItemEdit => "content_worklist_item_edit",
            Self::WorklistMarkdown => "content_worklist_markdown",
            Self::WorklistEmpty => "content_worklist_empty",
            Self::WorklistEmptyLight => "content_worklist_empty_light",
            Self::WorklistEmptyVirtual => "content_worklist_empty_virtual",
            Self::WorklistInlineCapture => "content_worklist_inline_capture",
            Self::WorklistInlineLight => "content_worklist_inline_light",
            Self::WorklistSections => "content_worklist_sections",
            Self::WorklistSectionsLight => "content_worklist_sections_light",
            Self::WorklistSectionsFolded => "content_worklist_sections_folded",
            Self::WorklistSectionRename => "content_worklist_section_rename",
            Self::WorklistCaptureSection => "overlay_worklist_capture_section",
            Self::WorklistToolbarTooltip => "overlay_worklist_toolbar_tooltip",
            Self::ToastSuccess => "overlay_toast_success",
            Self::ToastInfo => "overlay_toast_info",
            Self::ToastError => "overlay_toast_error",
            Self::ToastErrorLight => "overlay_toast_error_light",
            Self::ToastKeymapError => "overlay_toast_keymap_error",
            Self::ToastLongKeys => "overlay_toast_long_keys",
            Self::ConnectPhone => "overlay_connect_phone",
            Self::ConnectPhoneOffline => "overlay_connect_phone_no_tailscale",
            Self::PhoneDriving => "content_terminal_phone_fit",
            Self::SettingsRemote => "settings_remote",
            Self::SettingsEditor => "settings_editor",
            Self::SettingsLanguageServers => "settings_language_servers",
            Self::SettingsSearch => "settings_search",
            Self::SettingsLight => "settings_window_light",
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
        Scene::SettingsDropdownFiltered => settings::dropdown_filtered(app, window, cx),
        Scene::TabTooltip => overlays::tab_tooltip(app, window, cx),
        Scene::SettingsWindow
        | Scene::SettingsEditor
        | Scene::SettingsLanguageServers
        | Scene::SettingsSearch
        | Scene::SettingsLight => settings::page(app, scene, cx),
        Scene::SkillPromptDark | Scene::SkillPromptLight => {
            overlays::skill_prompt(app, scene, window, cx)
        }
        Scene::SettingsAgentsInstalled => settings::agents_installed(app, cx),
        Scene::ConnectPhone => remote::connect_phone(app, true, window, cx),
        Scene::ConnectPhoneOffline => remote::connect_phone(app, false, window, cx),
        Scene::PhoneDriving => content::phone_driving(app, window, cx),
        Scene::SettingsRemote => settings::remote(app, cx),
        Scene::WorklistCapture
        | Scene::WorklistCaptureFilled
        | Scene::WorklistCaptureLong
        | Scene::WorklistCaptureSection => worklist::capture(app, scene, window, cx),
        Scene::WorklistTab
        | Scene::WorklistItemEdit
        | Scene::WorklistMarkdown
        | Scene::WorklistEmpty
        | Scene::WorklistEmptyLight
        | Scene::WorklistEmptyVirtual
        | Scene::WorklistInlineCapture
        | Scene::WorklistInlineLight
        | Scene::WorklistSections
        | Scene::WorklistSectionsLight
        | Scene::WorklistSectionsFolded
        | Scene::WorklistSectionRename => worklist::tab(app, scene, window, cx),
        Scene::WorklistToolbarTooltip => chrome::populated(app, scene, window, cx),
        Scene::ToastSuccess
        | Scene::ToastInfo
        | Scene::ToastError
        | Scene::ToastErrorLight
        | Scene::ToastKeymapError
        | Scene::ToastLongKeys => {
            chrome::populated(app, scene, window, cx);
            let toast = match scene {
                Scene::ToastKeymapError => super::toasts::keymap_error(
                    "binding \"ctrl-x\", didn’t find an action named \"xenon::NoSuchThing\".",
                ),
                Scene::ToastLongKeys => super::toasts::long_keys_sample(),
                Scene::ToastSuccess => {
                    super::toasts::task_added(Some("Fix focus after closing a split"))
                }
                Scene::ToastInfo => super::toasts::no_workspace(),
                _ => super::toasts::worklist_error(
                    "Can’t undo just yet",
                    "Save or undo the worklist Markdown edits first",
                ),
            };
            app.show_toast(toast, cx);
        }
    }
}

impl XenonApp {
    /// Runner hook after the scene's terminal is ready (for PTY-dependent state).
    pub fn visual_after_terminal_ready(&mut self, scene: Scene, cx: &mut Context<XenonApp>) {
        if scene == Scene::PhoneDriving {
            self.visual_phone_driving(cx);
        }
    }

    pub fn visual_settings_window(&self) -> Option<WindowHandle<SettingsView>> {
        self.settings_window
    }

    pub fn visual_terminal_ready(&self, cx: &gpui::App) -> bool {
        self.active_terminal()
            .is_some_and(|view| view.read(cx).visual_is_ready())
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

fn theme_for(scene: Scene, cx: &mut Context<XenonApp>) {
    match scene {
        Scene::EmptyNoWorkspaceLight
        | Scene::EmptyWithWorkspaceLight
        | Scene::PopulatedLight
        | Scene::SkillPromptLight
        | Scene::WorklistInlineLight
        | Scene::WorklistSectionsLight
        | Scene::WorklistEmptyLight
        | Scene::ToastErrorLight => {
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
