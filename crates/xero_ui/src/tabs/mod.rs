//! Editor and terminal tab strips. Multi-channel selection (fill + type +
//! accent underline). Dirty markers on editor chips; exited terminals use
//! theme status tint (no fake close ✗).

mod menu;

use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;

use crate::{
    app::{TabSurface, XeroApp},
    chrome::{self, SelectionPaint},
    preview_icon,
};

fn tab_underline(paint: SelectionPaint) -> impl IntoElement {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(paint.accent)
}

fn dirty_dot(color: gpui::Hsla) -> impl IntoElement {
    div()
        .w(px(6.))
        .h(px(6.))
        .rounded_full()
        .bg(color)
        .flex_none()
}

fn tab_close(
    id: impl Into<gpui::ElementId>,
    group: &str,
    colors: &theme::ThemeColors,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let group = group.to_string();
    div()
        .id(id)
        .text_xs()
        .text_color(colors.text_muted)
        .invisible()
        .group_hover(group, |s| s.visible())
        .hover(|s| s.text_color(colors.text))
        .child("✕")
        .on_click(on_click)
}

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

struct TabChip<'a> {
    index: usize,
    name: &'a str,
    is_active: bool,
    is_dirty: bool,
}

struct TerminalChip<'a> {
    index: usize,
    title: &'a str,
    is_active: bool,
    is_exited: bool,
}

impl XeroApp {
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let tabs: Vec<(usize, String, bool, bool)> = self
            .editor_stack()
            .map(|stack| {
                stack
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(i, tab)| {
                        let dirty = tab.view.read(cx).is_dirty();
                        (i, tab.name.clone(), i == stack.active, dirty)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut chips = Vec::with_capacity(tabs.len());
        for (index, name, is_active, is_dirty) in tabs {
            chips.push(self.tab_chip(
                TabChip {
                    index,
                    name: &name,
                    is_active,
                    is_dirty,
                },
                cx,
            ));
        }

        let previewing = self.active_editor_is_previewing(cx);
        let preview = self.active_editor_is_markdown(cx).then(|| {
            div()
                .id("md-preview-toggle")
                .ml_auto()
                .w(px(30.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(if previewing {
                    colors.text
                } else {
                    colors.text_muted
                })
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                .child(preview_icon(previewing))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx)))
        });

        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(chrome::tab_bar_background(&colors))
            .children(chips)
            .children(preview)
    }

    fn tab_chip(&self, chip: TabChip, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let TabChip {
            index,
            name,
            is_active,
            is_dirty,
        } = chip;
        let colors = cx.theme().colors().clone();
        let paint = chrome::tab_selection(&colors, is_active);
        let group = format!("editor-tab-{index}");
        div()
            .id(("tab", index))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .bg(paint.background)
            .text_color(paint.foreground)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, _, cx| this.activate_tab(index, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_tab_menu(TabSurface::Editor, index, event.position, cx);
                }),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_active {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .child(name.to_string()),
            )
            .children(is_dirty.then(|| dirty_dot(paint.foreground)))
            .child(tab_close(
                ("tab-close", index),
                &group,
                &colors,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_tab(index, window, cx);
                }),
            ))
            .child(tab_underline(paint))
    }
}

impl XeroApp {
    /// Terminal tab strip; labels use program-set titles. `+` adds a terminal.
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
            chips.push(self.terminal_chip(
                TerminalChip {
                    index,
                    title: &title,
                    is_active,
                    is_exited,
                },
                cx,
            ));
        }

        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(chrome::tab_bar_background(&colors))
            .children(chips)
            .child(
                div()
                    .id("term-add")
                    .px_2()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                    .child("+")
                    .on_click(cx.listener(|this, _, _, cx| this.add_terminal(cx))),
            )
    }

    fn terminal_chip(
        &self,
        chip: TerminalChip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let TerminalChip {
            index,
            title,
            is_active,
            is_exited,
        } = chip;
        let colors = cx.theme().colors().clone();
        let status = cx.theme().status().clone();
        let paint = chrome::tab_selection(&colors, is_active);
        // Exited: theme ignored tint so the tab reads "done", not a second close control.
        let (bg, label_color, underline) = if is_exited {
            (
                status.ignored_background,
                status.ignored,
                status.ignored_border,
            )
        } else {
            (paint.background, paint.foreground, paint.accent)
        };
        let paint = chrome::SelectionPaint {
            background: bg,
            foreground: label_color,
            accent: underline,
        };
        let group = format!("term-tab-{index}");
        let tip = if is_exited {
            format!("{title} — process exited")
        } else {
            title.to_string()
        };
        div()
            .id(("term-tab", index))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .border_r_1()
            .border_color(if is_exited {
                status.ignored_border
            } else {
                colors.border
            })
            .bg(paint.background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(
                cx.listener(move |this, _, window, cx| {
                    this.activate_terminal_tab(index, window, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_tab_menu(TabSurface::Terminal, index, event.position, cx);
                }),
            )
            .tooltip({
                let full = SharedString::from(tip);
                move |_window: &mut Window, cx: &mut App| {
                    cx.new(|_| TabTooltip { text: full.clone() }).into()
                }
            })
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_active {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .text_color(paint.foreground)
                    .max_w(px(220.))
                    .truncate()
                    .child(title.to_string()),
            )
            .child(tab_close(("term-close", index), &group, &colors, {
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_terminal_tab(index, window, cx);
                })
            }))
            .child(tab_underline(paint))
    }
}
