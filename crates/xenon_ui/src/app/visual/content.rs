use gpui::{Context, Window, px};

use xenon_store::ThemeMode;

use super::{Scene, XenonApp, ensure_terminal, open_rel, populate};

pub(super) fn terminal_grid(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    ensure_terminal(app, window, cx);
    focus_terminal_tab(app, window, cx);
    cx.notify();
}

pub(super) fn phone_driving(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    terminal_grid(app, window, cx);
    app.visual_remote(true, cx);
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

pub(super) fn editor_wrap(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    open_rel(app, "WRAP.md", window, cx);
    cx.notify();
}

pub(super) fn markdown_preview(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    open_rel(app, "../docs/GUIDE.md", window, cx);
    cx.notify();
}

/// The Markdown writing view on a substack-shaped essay.
pub(super) fn prose(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    use xenon_settings::{MarkdownMode, ProseMarkers};
    let theme = if scene == Scene::ProseDark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    super::set_theme(theme, "One Dark", "One Light", cx);
    xenon_settings::update_prose(cx, |p| {
        p.markers = if scene == Scene::ProseHideMarkers {
            ProseMarkers::Hide
        } else {
            ProseMarkers::Dim
        };
    });
    populate(app, window, cx);
    open_rel(app, "ESSAY.md", window, cx);
    if let Some(editor) = app.active_editor() {
        editor.update(cx, |editor, cx| {
            editor.set_markdown_mode(
                if scene == Scene::ProseSplit {
                    MarkdownMode::Split
                } else {
                    MarkdownMode::Prose
                },
                cx,
            );
            if editor.is_paper() == (scene == Scene::ProseNoPaper) {
                editor.toggle_paper(cx);
            }
            if editor.comments_open() == (scene != Scene::ProseCommentsOpen) {
                editor.toggle_comments_fold(cx);
            }
            if scene == Scene::ProseFullScreen {
                editor.toggle_full_screen(cx);
            }
            if matches!(scene, Scene::ProseLight | Scene::ProseDark) {
                // Heading word into the body, over bold: the Paper selection fill.
                editor.set_selection_range((4, 16), (7, 30), cx);
            }
        });
    }
    cx.notify();
}

/// Narrow pane: the outline hides and the column shrinks to fit.
pub(super) fn markdown_preview_split(
    app: &mut XenonApp,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    markdown_preview(app, window, cx);
    app.split_right(window, cx);
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
