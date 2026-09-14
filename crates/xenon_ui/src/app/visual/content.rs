use gpui::{Context, Window, px};

use super::{XenonApp, ensure_terminal, open_rel, populate};

pub(super) fn terminal_grid(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    ensure_terminal(app, window, cx);
    focus_terminal_tab(app, window, cx);
    cx.notify();
}

pub(super) fn focus_terminal_tab(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    if let Some((pane, index)) = terminal_tab_index(app) {
        app.activate_tab_in_pane(pane, index, window, cx);
    }
    app.deferred.pending_focus = None;
}

fn terminal_tab_index(app: &XenonApp) -> Option<(xenon_core::PaneId, usize)> {
    let content = app.active_content()?;
    let leaf = content.focused_leaf()?;
    let index = leaf.tabs.iter().rposition(|tab| tab.is_terminal())?;
    Some((leaf.id, index))
}

pub(super) fn editor_highlight(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    open_rel(app, "src/main.rs", window, cx);
    cx.notify();
}

pub(super) fn markdown_preview(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    open_rel(app, "NOTES.md", window, cx);
    app.toggle_preview(cx);
    cx.notify();
}

pub(super) fn image_viewer(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "logo.png", window, cx);
    cx.notify();
}

pub(super) fn unsupported(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "blob.bin", window, cx);
    cx.notify();
}

pub(super) fn editor_find(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    editor_highlight(app, window, cx);
    if let Some(editor) = app.active_editor() {
        editor.update(cx, |editor, cx| editor.open_find(window, cx));
    }
    cx.notify();
}

pub(super) fn terminal_find(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    terminal_grid(app, window, cx);
    if let Some(terminal) = app.active_terminal() {
        terminal.update(cx, |terminal, cx| {
            terminal.open_find(window, cx);
        });
    }
    cx.notify();
}

pub(super) fn menu_point() -> gpui::Point<gpui::Pixels> {
    gpui::point(px(420.), px(180.))
}
