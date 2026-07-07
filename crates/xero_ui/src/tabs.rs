//! The editor tab strip above the editor pane. One chip per open file, with a
//! close affordance; clicking a chip focuses that tab.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use theme::ActiveTheme;

use crate::app::XeroApp;

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

        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .children(chips)
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
        let tabs: Vec<(usize, String, bool)> = self
            .terminal_stack()
            .map(|stack| {
                stack
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, view)| (i, view.read(cx).title(cx), i == stack.active))
                    .collect()
            })
            .unwrap_or_default();

        let mut chips = Vec::with_capacity(tabs.len());
        for (index, title, is_active) in tabs {
            chips.push(self.terminal_chip(index, &title, is_active, cx));
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
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let background =
            if is_active { colors.terminal_background } else { colors.panel_background };
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
            .child(div().text_sm().max_w(px(220.)).truncate().child(title.to_string()))
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
