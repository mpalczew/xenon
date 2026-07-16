use super::*;
use crate::RunTask;
use crate::Save;
use crate::resize::ResizeEdge;
use crate::{
    CloseStream, CloseWorkspace, CommandPalette, FocusBrowser, FocusEditor, FocusNextPane,
    FocusTerminal, KeyboardHelp, MoveTabMenu, NextStream, NextTab, NextWorkspace, PrevStream,
    PrevTab, PrevWorkspace, StreamPalette,
};
use gpui::{AnyElement, DragMoveEvent, KeyDownEvent, MouseButton, MouseUpEvent};
use xero_settings::{Copy, Cut, Paste};

use super::empty_hint::empty_editor_hint;

impl Render for XeroApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.window_title());
        self.drain_deferred_ui(window, cx);
        let ui = xero_settings::ui_font(cx);
        // rem drives text_sm/xs/lg across chrome; family cascades to children.
        window.set_rem_size(px(ui.size));
        let colors = cx.theme().colors().clone();
        let toolbar = self.render_toolbar(cx);
        let sidebar = (!self.sidebar_collapsed).then(|| self.render_sidebar(cx));
        let main = self.render_main(window, cx);
        let finder = self.finder.clone();
        let task_picker = self.task_picker.clone();
        let workspace_picker = self.workspace_picker.clone();
        let command_palette = self.command_palette.clone();
        let tab_menu = self.render_tab_menu(cx);
        let body = self.render_shell_body(sidebar, main, colors.clone(), cx);
        self.bind_app_actions(div(), cx)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.on_tab_menu_key(event, window, cx) || this.on_browser_key(event, window, cx)
                {
                    cx.stop_propagation();
                }
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
            .font_family(ui.family)
            .child(toolbar)
            .child(body)
            .children(finder)
            .children(task_picker)
            .children(workspace_picker)
            .children(command_palette)
            .children(tab_menu)
    }
}

impl XeroApp {
    /// Deferred work that needs a Window (finder dismiss focus, cmd-click palette).
    fn drain_deferred_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.pending_focus.take() {
            self.focus_pane(pane, window, cx);
        }
        if let Some(query) = self.pending_palette_query.take() {
            self.open_palette_with_query(query, window, cx);
        }
        if let Some(id) = self.pending_stream.take() {
            self.select_stream(id, window, cx);
        }
        if let Some(cmd) = self.pending_command.take() {
            self.run_command(cmd, window, cx);
        }
    }

    fn render_shell_body(
        &self,
        sidebar: Option<impl IntoElement + 'static>,
        main: impl IntoElement + 'static,
        colors: theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let reopen_sidebar = !self.sidebar_visible();
        div()
            .relative()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .on_drag_move(cx.listener(Self::on_sidebar_drag))
            .children(sidebar)
            .child(main)
            .children(reopen_sidebar.then(|| {
                crate::resize::col_resize_handle_at(
                    "sidebar-reopen",
                    ResizeEdge::Sidebar,
                    colors.border,
                    crate::resize::HandleSide::Left,
                )
            }))
            .into_any_element()
    }

    fn bind_app_actions(&self, root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let root = self.bind_core_actions(root, cx);
        self.bind_nav_actions(root, cx)
    }

    fn bind_core_actions(&self, root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        root.track_focus(&self.focus)
            .key_context("XeroApp")
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &NewTerminal, window, cx| {
                this.new_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewStream, window, cx| {
                this.new_stream(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.open_file_dialog(cx)))
            .on_action(
                cx.listener(|this, _: &AddWorkspace, window, cx| this.add_workspace(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FilePalette, window, cx| this.open_palette(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &RunTask, window, cx| this.open_task_picker(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleBrowser, _, cx| {
                this.browser_focused = false;
                this.toggle_browser(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal_panel(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleEditor, _, cx| {
                this.toggle_editor_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.toggle_settings_window(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                this.close_focused_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save_active_editor(cx)))
            .on_action(cx.listener(|this, _: &IncreaseFontSize, window, cx| {
                this.nudge_font_size(1.0, window, cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseFontSize, window, cx| {
                this.nudge_font_size(-1.0, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ResetFontSize, window, cx| {
                this.reset_font_size(window, cx);
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
    }

    fn bind_nav_actions(&self, root: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        root.on_action(cx.listener(|this, _: &FocusTerminal, window, cx| {
            this.focus_terminal(window, cx);
        }))
        .on_action(cx.listener(|this, _: &FocusEditor, window, cx| {
            this.focus_editor(window, cx);
        }))
        .on_action(cx.listener(|this, _: &FocusBrowser, window, cx| {
            this.focus_browser(window, cx);
        }))
        .on_action(cx.listener(|this, _: &FocusNextPane, window, cx| {
            this.focus_next_pane(window, cx);
        }))
        .on_action(cx.listener(|this, _: &NextStream, window, cx| {
            this.next_stream(window, cx);
        }))
        .on_action(cx.listener(|this, _: &PrevStream, window, cx| {
            this.prev_stream(window, cx);
        }))
        .on_action(cx.listener(|this, _: &NextWorkspace, window, cx| {
            this.next_workspace(window, cx);
        }))
        .on_action(cx.listener(|this, _: &PrevWorkspace, window, cx| {
            this.prev_workspace(window, cx);
        }))
        .on_action(cx.listener(|this, _: &StreamPalette, window, cx| {
            this.open_stream_palette(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CloseStream, window, cx| {
            this.close_active_stream(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CloseWorkspace, window, cx| {
            this.close_active_workspace(window, cx);
        }))
        .on_action(cx.listener(|this, _: &NextTab, window, cx| {
            this.next_tab(window, cx);
        }))
        .on_action(cx.listener(|this, _: &PrevTab, window, cx| {
            this.prev_tab(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
            this.open_command_palette(window, cx);
        }))
        .on_action(cx.listener(|this, _: &KeyboardHelp, window, cx| {
            this.open_keyboard_help(window, cx);
        }))
        .on_action(cx.listener(|this, _: &MoveTabMenu, window, cx| {
            this.open_move_tab_menu(window, cx);
        }))
    }
}

impl XeroApp {
    fn window_title(&self) -> String {
        let mut title = "Xenon".to_string();
        if let Some(stream) = self.active
            && let Some(workspace) = self.workspace_of(stream)
        {
            title = format!("Xenon - {} / {}", workspace.name, self.stream_name(stream));
        }
        let slot = std::env::var("XENON_SLOT")
            .or_else(|_| std::env::var("XERO_SLOT"))
            .ok();
        match slot {
            Some(slot) if !slot.trim().is_empty() => format!("{} {title}", slot.trim()),
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
        let terminal = self
            .terminal_visible()
            .then(|| self.active_terminal())
            .flatten();
        let show_terminal = self.terminal_visible();
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
        let both = show_terminal && self.editor_visible();
        let terminal_pane = show_terminal.then(|| {
            self.render_terminal_pane(
                PaneFrame {
                    split: both,
                    ring: focus_ring(term_focused, &colors),
                },
                terminal,
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
            (None, Some(right)) => {
                // Terminal snap-closed: left residual handle drags it back on.
                panel = panel
                    .relative()
                    .child(crate::resize::col_resize_handle_at(
                        "terminal-reopen",
                        ResizeEdge::Terminal,
                        colors.border,
                        crate::resize::HandleSide::Left,
                    ))
                    .child(right);
            }
            _ => {
                let message = if self.active.is_some() {
                    "Show Terminal or Editor from the toolbar"
                } else {
                    "Add a workspace (⌘⇧O), then a stream for each agent"
                };
                panel = panel
                    .items_center()
                    .justify_center()
                    .text_color(colors.text_muted)
                    .child(message);
            }
        }
        panel
    }

    fn render_terminal_pane(
        &self,
        frame: PaneFrame,
        terminal: Option<Entity<TerminalView>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let tabs = self.render_terminal_tabs(cx);
        let body = match terminal {
            Some(term) => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(term)
                .into_any_element(),
            None => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .bg(colors.background)
                .text_color(colors.text_muted)
                .text_sm()
                .child("No terminal open")
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child("Press ⌘N for a new terminal"),
                )
                .into_any_element(),
        };
        let mut pane = div()
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .border_2()
            .border_color(frame.ring)
            .child(tabs)
            .child(body);
        if frame.split {
            pane = pane.w(px(self.terminal_width_px())).flex_none().child(
                crate::resize::col_resize_handle(
                    "terminal-resize",
                    ResizeEdge::Terminal,
                    colors.border,
                ),
            );
        } else {
            // Editor snap-closed: right residual handle drags it back on.
            pane = pane.flex_1().child(crate::resize::col_resize_handle(
                "editor-reopen",
                ResizeEdge::Terminal,
                colors.border,
            ));
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
        let reopen_tree = self.active.is_some() && !self.is_browsing();
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
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_drag_move(cx.listener(Self::on_tree_drag))
                    .children(tree)
                    .child(editor_body(active_view, colors))
                    .children(reopen_tree.then(|| {
                        crate::resize::col_resize_handle_at(
                            "tree-reopen",
                            ResizeEdge::Tree,
                            colors.border,
                            crate::resize::HandleSide::Left,
                        )
                    })),
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
        None => empty_editor_hint(colors.text_muted),
    }
}
