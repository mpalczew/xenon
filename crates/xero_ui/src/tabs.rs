//! The editor tab strip above the editor pane. One chip per open file, with a
//! close affordance; clicking a chip focuses that tab.

use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;

use crate::app::XeroApp;

/// A hover tooltip showing a terminal tab's full (untruncated) title.
struct TabTooltip {
    text: SharedString,
}

impl Render for TabTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}

impl XeroApp {
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        // Snapshot tab labels/active state so the borrow is released before the
        // per-tab `cx.listener` closures.
        let tabs: Vec<(usize, String, bool)> = self
            .editor_stack()
            .map(|stack| {
                stack
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, tab)| (i, tab.name.clone(), i == stack.active))
                    .collect()
            })
            .unwrap_or_default();

        let mut chips = Vec::with_capacity(tabs.len());
        for (index, name, is_active) in tabs {
            chips.push(self.tab_chip(index, &name, is_active, cx));
        }

        // Right-aligned controls: Files (browser) always, Preview for markdown.
        let button = |id: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .px_3()
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .text_color(if active { colors.text } else { colors.text_muted })
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                .child(label)
        };
        let files = button("browse-toggle", "Files", self.is_browsing())
            .ml_auto()
            .on_click(cx.listener(|this, _, _, cx| this.toggle_browser(cx)));
        // "Reveal" shows the open file's location in the tree; only while editing.
        let reveal = (!self.is_browsing()).then(|| {
            button("reveal-file", "Reveal", false)
                .on_click(cx.listener(|this, _, _, cx| this.reveal_current_file(cx)))
        });
        let preview = self.active_editor_is_markdown(cx).then(|| {
            button("md-preview-toggle", "Preview", false)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx)))
        });

        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .children(chips)
            .child(files)
            .children(reveal)
            .children(preview)
    }

    fn tab_chip(
        &self,
        index: usize,
        name: &str,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let background =
            if is_active { colors.editor_background } else { colors.panel_background };
        div()
            .id(("tab", index))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .bg(background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, _, cx| this.activate_tab(index, cx)))
            .child(div().text_sm().child(name.to_string()))
            .child(
                div()
                    .id(("tab-close", index))
                    .text_xs()
                    .text_color(colors.text_muted)
                    .hover(|s| s.text_color(colors.text))
                    .child("✕")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, cx);
                    })),
            )
    }
}

impl XeroApp {
    /// The terminal tab strip; each tab is labeled with the terminal's title
    /// (which programs like Claude Code set to show status). `+` adds a terminal.
    pub(crate) fn render_terminal_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let tabs: Vec<(usize, String, bool, bool)> = self
            .terminal_stack()
            .map(|stack| {
                stack
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, view)| {
                        let view = view.read(cx);
                        (i, view.title(cx), i == stack.active, view.is_exited())
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut chips = Vec::with_capacity(tabs.len());
        for (index, title, is_active, is_exited) in tabs {
            chips.push(self.terminal_chip(index, &title, is_active, is_exited, cx));
        }

        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .children(chips)
            .child(
                div()
                    .id("term-add")
                    .px_2()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child("+")
                    .on_click(cx.listener(|this, _, _, cx| this.add_terminal(cx))),
            )
    }

    fn terminal_chip(
        &self,
        index: usize,
        title: &str,
        is_active: bool,
        is_exited: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let background =
            if is_active { colors.terminal_background } else { colors.panel_background };
        // Dead terminals get a dim ✗ and muted label.
        let label_color = if is_exited { colors.text_muted } else { colors.text };
        let dead = is_exited.then(|| div().text_xs().text_color(colors.text_muted).child("✗"));
        div()
            .id(("term-tab", index))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .bg(background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_terminal_tab(index, window, cx)
            }))
            .tooltip({
                let full = SharedString::from(title.to_string());
                move |_window: &mut Window, cx: &mut App| {
                    cx.new(|_| TabTooltip { text: full.clone() }).into()
                }
            })
            .children(dead)
            .child(
                div()
                    .text_sm()
                    .text_color(label_color)
                    .max_w(px(220.))
                    .truncate()
                    .child(title.to_string()),
            )
            .child(
                div()
                    .id(("term-close", index))
                    .text_xs()
                    .text_color(colors.text_muted)
                    .hover(|s| s.text_color(colors.text))
                    .child("✕")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_terminal_tab(index, window, cx);
                    })),
            )
    }
}
