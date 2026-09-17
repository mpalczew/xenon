use gpui::{Context, Window};
use xenon_core::{Registry, SplitAxis};

use super::{Scene, XenonApp, ensure_terminal, open_rel, populate, theme_for};
use crate::app::AttentionReason;
use crate::git_dirt::GitDirt;

pub(super) fn empty_no_workspace(app: &mut XenonApp, scene: Scene, cx: &mut Context<XenonApp>) {
    app.registry = Registry::default();
    app.active = None;
    app.contents.clear();
    app.sessions.clear();
    theme_for(scene, cx);
    cx.notify();
}

pub(super) fn empty_with_workspace(_app: &mut XenonApp, scene: Scene, cx: &mut Context<XenonApp>) {
    theme_for(scene, cx);
    cx.notify();
}

pub(super) fn populated(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    theme_for(scene, cx);
    populate(app, window, cx);
    cx.notify();
}

pub(super) fn sidebar_attention(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    seed_second_workspace_terminal(app, window, cx, false);
    cx.notify();
}

pub(super) fn sidebar_working(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    seed_second_workspace_terminal(app, window, cx, true);
    cx.notify();
}

pub(super) fn files_selected(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.file_browser.open();
    if let Some(root) = super::workspace_root(app) {
        app.file_browser.reveal_dir(&root, &root.join("src"));
        let rows = app
            .file_browser
            .rows(&root, Some(&root.join("src/main.rs")));
        app.file_browser
            .select_path_in(&rows, &root.join("src/main.rs"));
    }
    app.focus_browser(window, cx);
    app.deferred.pending_focus = None;
}

pub(super) fn files_git_dirt(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    files_selected(app, window, cx);
    if let Some(id) = app.active {
        app.services.git_dirt.insert(
            id,
            GitDirt {
                insertions: 12,
                deletions: 3,
            },
        );
    }
    cx.notify();
}

pub(super) fn tabs_dirty(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    if let Some(editor) = app.active_editor() {
        editor.update(cx, |editor, cx| {
            editor.visual_insert(" // dirty", cx);
        });
    }
    cx.notify();
}

pub(super) fn tabs_overflow(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
    open: bool,
) {
    populate(app, window, cx);
    for rel in [
        "src/settings.rs",
        "src/boot.rs",
        "src/gemini.rs",
        "src/grok.rs",
        "src/login.rs",
        "src/panels.rs",
        "src/agent.rs",
        "src/claude.rs",
        "src/xenon.rs",
        "src/session.rs",
        "src/content.rs",
        "src/keyboard.rs",
    ] {
        open_rel(app, rel, window, cx);
    }
    ensure_terminal(app, window, cx);
    if let Some(id) = app.active
        && let Some((_, tab)) = first_terminal_tab(app)
    {
        app.flag_attention(id, tab, AttentionReason::Bell, cx);
    }
    if let Some((pane, index)) = notes_tab(app) {
        app.activate_tab_in_pane(pane, index, window, cx);
        if open {
            app.toggle_overflow_menu(pane, gpui::point(gpui::px(980.), gpui::px(48.)), cx);
        }
    }
    cx.notify();
}

fn notes_tab(app: &XenonApp) -> Option<(xenon_core::PaneId, usize)> {
    let content = app.active_content()?;
    let leaf = content.focused_leaf()?;
    let index = leaf.tabs.iter().position(|tab| {
        tab.editor_path()
            .is_some_and(|path| path.ends_with("NOTES.md"))
    })?;
    Some((leaf.id, index))
}

pub(super) fn tabs_attention(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    ensure_terminal(app, window, cx);
    if let Some(id) = app.active
        && let Some((_, tab)) = first_terminal_tab(app)
    {
        app.flag_attention(id, tab, AttentionReason::Bell, cx);
    }
    cx.notify();
}

pub(super) fn split_right(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "NOTES.md", window, cx);
    app.split_right(window, cx);
    cx.notify();
}

pub(super) fn split_down(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "NOTES.md", window, cx);
    app.split_down(window, cx);
    cx.notify();
}

pub(super) fn reserved_empty(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.park_empty_pane(SplitAxis::Horizontal, window, cx);
    cx.notify();
}

pub(super) fn focus_ring(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    ensure_terminal(app, window, cx);
    super::content::focus_terminal_tab(app, window, cx);
    cx.notify();
}

fn first_terminal_tab(app: &XenonApp) -> Option<(xenon_core::PaneId, xenon_core::TabId)> {
    let content = app.active_content()?;
    let mut found = None;
    if let Some(root) = &content.root {
        root.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                for tab in &leaf.tabs {
                    if let crate::app::LiveTab::Terminal { id, .. } = tab {
                        found = Some((leaf.id, *id));
                        break;
                    }
                }
            }
        });
    }
    found
}

fn seed_second_workspace_terminal(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
    working: bool,
) {
    let Some(second) = app
        .registry
        .workspaces
        .iter()
        .map(|workspace| workspace.id)
        .find(|id| Some(*id) != app.active)
    else {
        ensure_terminal(app, window, cx);
        if let Some(id) = app.active
            && let Some((_, tab)) = first_terminal_tab(app)
        {
            if working {
                set_working(app, cx);
            } else {
                app.flag_attention(id, tab, AttentionReason::Bell, cx);
            }
        }
        return;
    };
    let demo = app.active;
    app.activate_workspace(second, cx);
    app.new_terminal(window, cx);
    if working {
        set_working(app, cx);
    } else if let Some((_, tab)) = first_terminal_tab(app) {
        app.flag_attention(second, tab, AttentionReason::IdleSettled, cx);
    }
    if let Some(demo) = demo {
        app.activate_workspace(demo, cx);
        populate(app, window, cx);
    }
}

fn set_working(app: &mut XenonApp, cx: &mut Context<XenonApp>) {
    if let Some(terminal) = app.active_terminal() {
        terminal.update(cx, |terminal, cx| {
            terminal.visual_set_working(true, cx);
        });
    }
}
