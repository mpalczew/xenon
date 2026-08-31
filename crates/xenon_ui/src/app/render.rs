use super::*;
use crate::resize::ResizeEdge;
use crate::{
    CloseWorkspace, CommandPalette, FocusBrowser, FocusEditor, FocusNextPane, FocusTerminal,
    GoBack, GoForward, GoToDefinition, KeyboardHelp, NextDiagnostic, NextTab, NextWorkspace,
    PrevTab, PrevWorkspace, PreviousDiagnostic, SplitDown, SplitRight,
};
use gpui::{AnyElement, DragMoveEvent, KeyDownEvent, MouseButton, MouseUpEvent, relative};
use xenon_core::{PaneId, SplitAxis};
use xenon_settings::{Copy, Cut, Paste};

use super::empty_hint::empty_editor_hint;

impl Render for XenonApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.window_title());
        self.drain_deferred_ui(window, cx);
        let ui = xenon_settings::ui_font(cx);
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
        let browser_menu = self.render_browser_menu(cx);
        let body = self.render_shell_body(sidebar, main, colors.clone(), cx);
        self.bind_app_actions(div(), cx)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.on_tab_menu_key(event, window, cx)
                    || this.on_browser_menu_key(event, window, cx)
                    || this.on_browser_key(event, window, cx)
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
            .children(browser_menu)
    }
}

impl XenonApp {
    fn drain_deferred_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.deferred.pending_leaf.take() {
            self.deferred.pending_focus = None;
            if self
                .active_content()
                .and_then(|c| c.root.as_ref()?.find_leaf(pane))
                .is_some()
            {
                self.focus_leaf_active(pane, window, cx);
            } else {
                self.focus_after_teardown(Some(window), cx);
            }
        } else if let Some(pane) = self.deferred.pending_focus.take() {
            self.focus_pane(pane, window, cx);
        }
        if let Some(query) = self.deferred.pending_palette_query.take() {
            self.open_palette_with_query(query, window, cx);
        }
        if let Some(id) = self.deferred.pending_workspace.take() {
            self.select_workspace(id, window, cx);
        }
        if let Some(cmd) = self.deferred.pending_command.take() {
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
            .key_context("XenonApp")
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &NewTerminal, window, cx| {
                this.new_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.open_file_dialog(cx)))
            .on_action(cx.listener(|this, _: &NewFile, _, cx| this.new_file_dialog(cx)))
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
            .on_action(cx.listener(|this, _: &crate::ToggleTerminal, window, cx| {
                this.focus_or_new_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &crate::ToggleEditor, window, cx| {
                this.focus_or_reveal_editor(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.split_right(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                this.split_down(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.toggle_settings_window(cx);
            }))
            .on_action(
                cx.listener(|this, _: &crate::ToggleMobileRemote, window, cx| {
                    this.toggle_mobile_remote(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &crate::TogglePreview, _, cx| {
                this.toggle_preview(cx);
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, window, cx| {
                this.close_focused_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.save_active_editor(cx)))
            .on_action(cx.listener(|this, _: &SaveAs, _, cx| this.save_as_dialog(cx)))
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
        .on_action(cx.listener(|this, _: &NextWorkspace, window, cx| {
            this.next_workspace(window, cx);
        }))
        .on_action(cx.listener(|this, _: &PrevWorkspace, window, cx| {
            this.prev_workspace(window, cx);
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
        .on_action(cx.listener(|this, _: &GoBack, window, cx| {
            this.go_back(window, cx);
        }))
        .on_action(cx.listener(|this, _: &GoForward, window, cx| {
            this.go_forward(window, cx);
        }))
        .on_action(cx.listener(|this, _: &GoToDefinition, _, cx| {
            this.go_to_definition(cx);
        }))
        .on_action(cx.listener(|this, _: &NextDiagnostic, _, cx| {
            this.next_diagnostic(true, cx);
        }))
        .on_action(cx.listener(|this, _: &PreviousDiagnostic, _, cx| {
            this.next_diagnostic(false, cx);
        }))
        .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
            this.open_command_palette(window, cx);
        }))
        .on_action(cx.listener(|this, _: &KeyboardHelp, window, cx| {
            this.open_keyboard_help(window, cx);
        }))
    }

    fn window_title(&self) -> String {
        let mut title = "Xenon".to_string();
        if let Some(id) = self.active
            && let Some(workspace) = self.registry.workspace(id)
        {
            title = format!("Xenon - {}", workspace.name);
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
        match *event.drag(cx) {
            ResizeEdge::Sidebar | ResizeEdge::SidebarSections => {
                let edge = *event.drag(cx);
                self.on_resize_drag(event, event.bounds.origin.x, edge, cx);
            }
            ResizeEdge::Content { .. } => {}
        }
    }

    fn on_content_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(*event.drag(cx), ResizeEdge::Content { .. }) {
            let edge = *event.drag(cx);
            self.on_resize_drag(event, event.bounds.origin.x, edge, cx);
        }
    }

    fn render_main(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let content = self.active_content();
        let mut panel = div()
            .flex()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .on_drag_move(cx.listener(Self::on_content_drag))
            // Keep drop overlays painted for the whole tab drag.
            .on_drag_move(cx.listener(|_, _: &DragMoveEvent<DragTab>, _, cx| {
                cx.notify();
            }));

        match content.and_then(|c| c.root.as_ref()) {
            Some(root) => {
                panel = panel.child(self.render_live_node(root, window, cx));
            }
            None => {
                let message = if self.active.is_some() {
                    "No open surfaces · ⌘N terminal · open a file"
                } else {
                    "Open a workspace (⌘⇧O)"
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

    fn render_live_node(
        &self,
        node: &LiveNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        match node {
            LiveNode::Leaf(leaf) => self.render_leaf(leaf, window, cx),
            LiveNode::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let first_leaf = first.leaf_ids().first().copied().unwrap_or(PaneId(0));
                let edge = ResizeEdge::Content {
                    axis: *axis,
                    first_leaf,
                };
                let first_el = self.render_live_node(first, window, cx);
                let second_el = self.render_live_node(second, window, cx);
                match axis {
                    SplitAxis::Horizontal => div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .child(
                            div()
                                .relative()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .min_h_0()
                                .w(relative(*ratio))
                                .child(first_el)
                                .child(crate::resize::col_resize_handle(
                                    format!("split-h-{}", first_leaf.0),
                                    edge,
                                    colors.border,
                                )),
                        )
                        .child(div().flex().flex_1().min_w_0().min_h_0().child(second_el))
                        .into_any_element(),
                    SplitAxis::Vertical => div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .child(
                            div()
                                .relative()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .min_h_0()
                                .h(relative(*ratio))
                                .child(first_el)
                                .child(crate::resize::row_resize_handle(
                                    format!("split-v-{}", first_leaf.0),
                                    edge,
                                    colors.border,
                                )),
                        )
                        .child(div().flex().flex_1().min_w_0().min_h_0().child(second_el))
                        .into_any_element(),
                }
            }
        }
    }

    fn render_leaf(&self, leaf: &LiveLeaf, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        // Ring tracks GPUI keyboard ownership, not the session leaf pointer.
        // AND-ing the two left splits with a purple tab underline, no pane
        // ring, and ⌘W closing the other half.
        let ring = if leaf
            .active_tab()
            .is_some_and(|tab| super::keyboard::tab_has_gpui_focus(tab, window, cx))
        {
            colors.border_focused
        } else {
            gpui::transparent_black()
        };

        let tabs = self.render_mixed_tabs(leaf, cx);
        let body = match leaf.active_tab() {
            Some(LiveTab::Terminal { view, .. }) => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(view.clone())
                .into_any_element(),
            Some(LiveTab::Editor { view, .. }) => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .child(view.clone())
                .into_any_element(),
            None => empty_editor_hint(colors.text_muted),
        };

        let pane_id = leaf.id;
        let ws = self.active;
        let dragging = cx.has_active_drag();
        let drop_line = colors.drop_target_border;

        // While a tab is dragged, overlay hit-targets so terminal/editor content
        // does not swallow the drop. Center = move; edges = split.
        let drop_overlay = dragging.then(|| self.tab_drop_overlay(pane_id, ws, drop_line, cx));

        div()
            .id(("leaf", leaf.id.0))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .border_2()
            .border_color(ring)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    // Tab strip is not track_focus; without this the root
                    // XenonApp handle steals GPUI and the pane ring goes away.
                    window.prevent_default();
                    this.focus_leaf_active(pane_id, window, cx);
                }),
            )
            .child(tabs)
            .child(body)
            .children(drop_overlay)
            .into_any_element()
    }
}
