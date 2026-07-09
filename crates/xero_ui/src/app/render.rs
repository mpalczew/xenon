use super::*;

impl Render for XeroApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.window_title());
        let colors = cx.theme().colors().clone();
        let toolbar = self.render_toolbar(cx);
        let sidebar = (!self.sidebar_collapsed).then(|| self.render_sidebar(cx));
        let main = self.render_main(window, cx);
        let finder = self.finder.clone();
        let settings = self.settings_open.then(|| self.render_settings_modal(cx));
        div()
            .track_focus(&self.focus)
            .key_context("XeroApp")
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.open_file_dialog(cx)))
            .on_action(cx.listener(|this, _: &AddWorkspace, _, cx| this.add_workspace(cx)))
            .on_action(cx.listener(|this, _: &FilePalette, _, cx| this.open_palette(cx)))
            .on_action(cx.listener(|this, _: &ToggleBrowser, _, cx| this.toggle_browser(cx)))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal_panel(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleEditor, _, cx| {
                this.toggle_editor_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _, cx| {
                this.settings_open = !this.settings_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CloseEditor, _, cx| this.close_editor(cx)))
            .on_action(cx.listener(|_, _: &IncreaseFontSize, window, cx| {
                xero_settings::adjust_font_size(cx, 1.0);
                window.refresh();
            }))
            .on_action(cx.listener(|_, _: &DecreaseFontSize, window, cx| {
                xero_settings::adjust_font_size(cx, -1.0);
                window.refresh();
            }))
            .on_action(cx.listener(|_, _: &ResetFontSize, window, cx| {
                xero_settings::reset_font_size(cx);
                window.refresh();
            }))
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
                    .children(sidebar)
                    .child(main),
            )
            .children(finder)
            .children(settings)
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

        let ring = |focused: bool| {
            if focused {
                colors.border_focused
            } else {
                gpui::transparent_black()
            }
        };
        let term_focused = terminal
            .as_ref()
            .is_some_and(|t| t.read(cx).focus_handle(cx).contains_focused(window, cx));
        let editor_focused = active_view
            .as_ref()
            .is_some_and(|e| e.read(cx).focus_handle(cx).contains_focused(window, cx));

        let terminal_tabs = terminal.is_some().then(|| self.render_terminal_tabs(cx));
        let editor_tabs =
            (self.editor_visible() && self.has_editor()).then(|| self.render_tab_bar(cx));
        let terminal_pane = terminal.map(|terminal| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .min_w_0()
                .border_2()
                .border_color(ring(term_focused))
                .children(terminal_tabs)
                .child(div().flex_1().min_h_0().child(terminal))
        });

        // Right pane = an optional file-tree sidebar beside the editor body. It
        // exists whenever an editor is open or the tree sidebar is toggled on.
        let tree_open = self.editor_visible() && self.is_browsing();
        let editor_body = match active_view {
            Some(view) => div().flex_1().min_h_0().child(view).into_any_element(),
            None => div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.text_muted)
                .child("Open a file from the tree or cmd-p")
                .into_any_element(),
        };
        let right_pane = self.editor_visible().then(|| {
            let tree = tree_open.then(|| self.render_tree_sidebar(cx));
            div()
                .flex_1()
                .flex()
                .flex_col()
                .min_w_0()
                .border_2()
                .border_color(ring(editor_focused))
                .children(editor_tabs)
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .children(tree)
                        .child(editor_body),
                )
        });

        let mut panel = div().flex().flex_1().size_full();
        match (terminal_pane, right_pane) {
            (Some(terminal_pane), Some(right_pane)) => {
                panel = panel.child(terminal_pane).child(right_pane);
            }
            (Some(terminal_pane), None) => {
                panel = panel.child(terminal_pane);
            }
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

    fn render_settings_modal(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let line_numbers = xero_settings::show_line_numbers(cx);
        div()
            .id("settings-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., 0.35))
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .id("settings-modal")
                    .occlude()
                    .w(px(360.))
                    .flex()
                    .flex_col()
                    .rounded_sm()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.elevated_surface_background)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(div().text_sm().text_color(colors.text).child("Settings"))
                            .child(
                                div()
                                    .id("settings-close")
                                    .w(px(24.))
                                    .h(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .text_color(colors.text_muted)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                                    .child("x")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.settings_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("line-number-toggle")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_3()
                            .cursor_pointer()
                            .hover(|s| s.bg(colors.element_hover))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(colors.text)
                                            .child("Line numbers"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .child("Show row numbers in text editors"),
                                    ),
                            )
                            .child(check_box(line_numbers, cx))
                            .on_click(cx.listener(|_, _, window, cx| {
                                xero_settings::toggle_line_numbers(cx);
                                window.refresh();
                                cx.notify();
                            })),
                    ),
            )
    }
}

fn check_box(checked: bool, cx: &mut Context<XeroApp>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let background = if checked {
        colors.element_selected
    } else {
        colors.elevated_surface_background
    };
    div()
        .w(px(18.))
        .h(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(background)
        .text_xs()
        .text_color(colors.text)
        .children(checked.then_some("x"))
}
