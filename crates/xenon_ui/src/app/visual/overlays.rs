use gpui::{Context, Window};
use xenon_core::PaneId;
use xenon_memory::ProcessMemory;

use super::super::RenameTarget;

use super::content::focus_terminal_tab;
use super::content::menu_point;
use super::{XenonApp, ensure_terminal, open_rel, populate, workspace_root};
use crate::app::memory::{MemorySample, MemorySnapshot, MemoryTerminal};
use crate::app::workspaces::WorkspacePickerMode;
use crate::dropdown::DropdownId;

pub(super) fn finder(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.open_palette(window, cx);
}

pub(super) fn command_palette(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.open_command_palette(window, cx);
}

pub(super) fn keyboard_help(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.open_keyboard_help(window, cx);
}

pub(super) fn task_picker(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.open_task_picker(window, cx);
}

pub(super) fn workspace_picker(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    app.open_workspace_picker(WorkspacePickerMode::Keyboard, window, cx);
}

pub(super) fn workspace_create(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    app.open_workspace_creator(window, cx);
}

pub(super) fn theme_gallery(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.open_theme_picker(window, cx);
}

pub(super) fn tab_menu(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    if let Some((pane, tab)) = focused_tab(app) {
        app.open_tab_menu(pane, tab, menu_point(), cx);
    }
}

pub(super) fn browser_menu(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    let path = workspace_root(app).map(|root| root.join("src/main.rs"));
    app.open_browser_menu(path, false, menu_point(), cx);
}

pub(super) fn workspace_menu(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.add_workspace_from_plus(window, cx);
}

pub(super) fn terminal_menu(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    ensure_terminal(app, window, cx);
    focus_terminal_tab(app, window, cx);
    if let Some(terminal) = app.active_terminal() {
        terminal.update(cx, |terminal, cx| {
            terminal.visual_open_menu(menu_point(), cx);
        });
    }
    cx.notify();
}

pub(super) fn editor_menu(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "src/main.rs", window, cx);
    if let Some(editor) = app.active_editor() {
        editor.update(cx, |editor, cx| {
            editor.visual_open_menu(menu_point(), cx);
        });
    }
}

pub(super) fn rename(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    let _ = window;
    if let Some(id) = app.active {
        app.begin_rename(RenameTarget::Workspace(id), "demo".into(), cx);
    }
}

pub(super) fn memory(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    let _ = window;
    app.memory_snapshot = Some(frozen_memory());
    app.memory_panel = true;
    cx.notify();
}

pub(super) fn settings_dropdown(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    let _ = window;
    app.open_settings_window_now(cx);
    if let Some(handle) = app.settings_window {
        let _ = handle.update(cx, |settings, window, cx| {
            settings.toggle_dropdown(DropdownId::EditorFamily, window, cx);
        });
    }
}

pub(super) fn tab_tooltip(app: &mut XenonApp, _window: &mut Window, _cx: &mut Context<XenonApp>) {
    populate(app, _window, _cx);
}

pub(super) fn settings_window(app: &mut XenonApp, cx: &mut Context<XenonApp>) {
    app.open_settings_window_now(cx);
}

fn focused_tab(app: &XenonApp) -> Option<(PaneId, xenon_core::TabId)> {
    let content = app.active_content()?;
    let leaf = content.focused_leaf()?;
    let tab = leaf.active_tab()?;
    Some((leaf.id, tab.id()))
}

fn frozen_memory() -> MemorySnapshot {
    MemorySnapshot {
        captured_at: "now".into(),
        process: ProcessMemory {
            footprint_bytes: 128 * 1024 * 1024,
            peak_footprint_bytes: 160 * 1024 * 1024,
            resident_bytes: 96 * 1024 * 1024,
            compressed_bytes: 12 * 1024 * 1024,
        },
        history: vec![MemorySample {
            captured_at: "now".into(),
            process: ProcessMemory {
                footprint_bytes: 128 * 1024 * 1024,
                peak_footprint_bytes: 160 * 1024 * 1024,
                resident_bytes: 96 * 1024 * 1024,
                compressed_bytes: 12 * 1024 * 1024,
            },
        }],
        terminals: vec![MemoryTerminal {
            tab_id: 1,
            title: "demo".into(),
            grid_bytes: 80 * 24 * 24,
            cols: 80,
            rows: 24,
            exited: false,
        }],
        terminal_estimate_bytes: 80 * 24 * 24,
    }
}
