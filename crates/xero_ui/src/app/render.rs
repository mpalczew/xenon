use super::*;
use crate::Save;
use crate::resize::ResizeEdge;
use gpui::{AnyElement, DragMoveEvent, MouseButton, MouseUpEvent};
use xero_settings::{Copy, Cut, Paste};

impl Render for XeroApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.window_title());
        // Restore focus to the pane the finder stole it from (deferred here from
        // the finder's Dismissed event, which has no Window).
        if let Some(pane) = self.pending_focus.take() {
            self.focus_pane(pane, window, cx);
        }
        // Open cmd-p prefilled from an ambiguous cmd-click (deferred from the
        // windowless terminal-event subscription).
        if let Some(query) = self.pending_palette_query.take() {
            self.open_palette_with_query(query, window, cx);
        }
        let colors = cx.theme().colors().clone();
        let toolbar = self.render_toolbar(cx);
        let sidebar = (!self.sidebar_collapsed).then(|| self.render_sidebar(cx));
        let main = self.render_main(window, cx);
        let finder = self.finder.clone();
        let tab_menu = self.render_tab_menu(cx);
        div()
            .track_focus(&self.focus)
            .key_context("XeroApp")
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.open_file_dialog(cx)))
            .on_action(cx.listener(|this, _: &AddWorkspace, _, cx| this.add_workspace(cx)))
            .on_action(
                cx.listener(|this, _: &FilePalette, window, cx| this.open_palette(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleBrowser, _, cx| this.toggle_browser(cx)))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal_panel(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleEditor, _, cx| {
                this.toggle_editor_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.toggle_settings_window(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, _, cx| this.close_editor(cx)))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save_active_editor(cx)))
            .on_action(cx.listener(|this, _: &IncreaseFontSize, window, cx| {
                this.nudge_font_size(1.0, window, cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseFontSize, window, cx| {
                this.nudge_font_size(-1.0, window, cx);
            }))
            .on_action(cx.listener(|_, _: &ResetFontSize, window, cx| {
                xero_settings::reset_font_sizes(cx);
                xero_settings::save(cx);
                window.refresh();
            }))
            .on_action(cx.listener(|this, _: &Cut, window, cx| {
                this.clipboard_cut(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Copy, window, cx| {
                this.clipboard_copy(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| {
                this.clipboard_paste(window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.finish_resize(cx)),
            )
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_drag_move(cx.listener(Self::on_sidebar_drag))
                    .children(sidebar)
                    .child(main),
            )
            .children(finder)
            .children(tab_menu)
    }
}

impl XeroApp {
    fn window_title(&self) -> String {
        let mut title = "xero".to_string();
        if let Some(stream) = self.active
            && let Some(workspace) = self.workspace_of(stream)
        {
            title = format!("xero - {} / {}", workspace.name, self.stream_name(stream));
        }
        match std::env::var("XERO_SLOT") {
            Ok(slot) if !slot.trim().is_empty() => format!("{} {title}", slot.trim()),
            _ => title,
        }
    }

    fn on_sidebar_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(*event.drag(cx), ResizeEdge::Sidebar) {
            self.on_resize_drag(event, event.bounds.origin.x, ResizeEdge::Sidebar, cx);
        }
    }

    fn on_tree_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(*event.drag(cx), ResizeEdge::Tree) {
            self.on_resize_drag(event, event.bounds.origin.x, ResizeEdge::Tree, cx);
        }
    }

    fn on_terminal_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(*event.drag(cx), ResizeEdge::Terminal) {
            self.on_resize_drag(event, event.bounds.origin.x, ResizeEdge::Terminal, cx);
        }
    }

    fn render_main(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let terminal = (!self.terminal_collapsed)
            .then(|| self.active_terminal())
            .flatten();
        let active_view = self.editor_visible().then(|| {
            self.editor_stack()
                .and_then(|stack| stack.tabs.get(stack.active))
                .map(|tab| tab.view.clone())
        });
        let active_view = active_view.flatten();
        let term_focused = terminal
            .as_ref()
            .is_some_and(|t| t.read(cx).focus_handle(cx).contains_focused(window, cx));
        let editor_focused = active_view
            .as_ref()
            .is_some_and(|e| e.read(cx).focus_handle(cx).contains_focused(window, cx));
        let both = terminal.is_some() && self.editor_visible();
        let terminal_pane = terminal.map(|t| {
            self.render_terminal_pane(
                PaneFrame {
                    split: both,
                    ring: focus_ring(term_focused, &colors),
                },
                t,
                cx,
            )
        });
        let right_pane = self.editor_visible().then(|| {
            self.render_editor_pane(
                focus_ring(editor_focused, &colors),
                active_view,
                &colors,
                cx,
            )
        });

        let mut panel = div()
            .flex()
            .flex_1()
            .size_full()
            .min_w_0()
            .on_drag_move(cx.listener(Self::on_terminal_drag));
        match (terminal_pane, right_pane) {
            (Some(term), Some(right)) => panel = panel.child(term).child(right),
            (Some(term), None) => panel = panel.child(term),
            _ => {
                let message = if self.active.is_some() {
                    "Use the toolbar to show a panel"
                } else {
                    "Add a workspace to begin"
                };
                panel = panel.items_center().justify_center().child(message);
            }
        }
        panel
    }

    fn render_terminal_pane(
        &self,
        frame: PaneFrame,
        terminal: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let tabs = self.render_terminal_tabs(cx);
        let mut pane = div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .border_2()
            .border_color(frame.ring)
            .child(tabs)
            .child(div().flex_1().min_h_0().min_w_0().child(terminal));
        if frame.split {
            pane = pane.w(px(self.terminal_width_px())).flex_none().child(
                crate::resize::col_resize_handle(
                    "terminal-resize",
                    ResizeEdge::Terminal,
                    colors.border,
                ),
            );
        } else {
            pane = pane.flex_1();
        }
        pane
    }

    fn render_editor_pane(
        &self,
        ring: gpui::Hsla,
        active_view: Option<Entity<EditorView>>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let tabs = self.has_editor().then(|| self.render_tab_bar(cx));
        let tree = self.is_browsing().then(|| self.render_tree_sidebar(cx));
        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_w_0()
            .border_2()
            .border_color(ring)
            .children(tabs)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_drag_move(cx.listener(Self::on_tree_drag))
                    .children(tree)
                    .child(editor_body(active_view, colors)),
            )
    }
}

struct PaneFrame {
    split: bool,
    ring: gpui::Hsla,
}

fn focus_ring(focused: bool, colors: &theme::ThemeColors) -> gpui::Hsla {
    if focused {
        colors.border_focused
    } else {
        gpui::transparent_black()
    }
}

fn editor_body(view: Option<Entity<EditorView>>, colors: &theme::ThemeColors) -> AnyElement {
    match view {
        Some(view) => div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(view)
            .into_any_element(),
        None => div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .text_color(colors.text_muted)
            .child("Open a file from the tree or cmd-p")
            .into_any_element(),
    }
}
