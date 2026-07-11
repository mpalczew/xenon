//! The editor tab strip above the editor pane. One chip per open file, with a
//! close affordance; clicking a chip focuses that tab. Right-click opens a
//! move-to-stream menu (shared with terminal tabs).

use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    anchored, deferred, div, px,
};
use theme::ActiveTheme;

use crate::{
    app::{TabContextMenu, TabMove, TabSurface, XeroApp},
    preview_icon,
};

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
            chips.push(self.tab_chip(
                TabChip {
                    index,
                    name: &name,
                    is_active,
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
            .bg(colors.panel_background)
            .children(chips)
            .children(preview)
    }

    fn tab_chip(&self, chip: TabChip, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let TabChip {
            index,
            name,
            is_active,
        } = chip;
        let colors = cx.theme().colors().clone();
        let background = if is_active {
            colors.editor_background
        } else {
            colors.panel_background
        };
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
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_tab_menu(TabSurface::Editor, index, event.position, cx);
                }),
            )
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
        let background = if is_active {
            colors.terminal_background
        } else {
            colors.panel_background
        };
        // Dead terminals get a dim ✗ and muted label.
        let label_color = if is_exited {
            colors.text_muted
        } else {
            colors.text
        };
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

impl XeroApp {
    pub(crate) fn open_tab_menu(
        &mut self,
        surface: TabSurface,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(stream) = self.active_stream() else {
            return;
        };
        self.tab_menu = Some(TabContextMenu {
            surface,
            stream,
            index,
            position,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn dismiss_tab_menu(&mut self, cx: &mut Context<Self>) {
        if self.tab_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Full-window overlay for the tab move menu, if open.
    pub(crate) fn render_tab_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let menu = self.tab_menu.as_ref()?;
        let siblings = self.sibling_streams(menu.stream);
        let colors = cx.theme().colors().clone();
        let surface = menu.surface;
        let stream = menu.stream;
        let index = menu.index;
        let position = menu.position;

        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(200.))
            .max_h(px(320.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(menu_item(
                "tab-move-new",
                "Move to New Stream",
                &colors,
                cx.listener(move |this, _, window, cx| {
                    this.dismiss_tab_menu(cx);
                    match surface {
                        TabSurface::Terminal => {
                            this.move_terminal_tab_to_new_stream(stream, index, window, cx)
                        }
                        TabSurface::Editor => {
                            this.move_editor_tab_to_new_stream(stream, index, window, cx)
                        }
                    }
                }),
            ));

        for (target, name) in siblings {
            let label = format!("Move to {name}");
            let id = format!("tab-move-{target}");
            menu_box = menu_box.child(menu_item(
                id,
                label,
                &colors,
                cx.listener(move |this, _, window, cx| {
                    this.dismiss_tab_menu(cx);
                    let tab = TabMove {
                        from: stream,
                        index,
                        to: target,
                    };
                    match surface {
                        TabSurface::Terminal => this.move_terminal_tab(tab, window, cx),
                        TabSurface::Editor => this.move_editor_tab(tab, window, cx),
                    }
                }),
            ));
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.dismiss_tab_menu(cx)),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| this.dismiss_tab_menu(cx)),
                )
                .child(deferred(anchored().position(position).child(menu_box)).with_priority(1)),
        )
    }
}

fn menu_item(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    colors: &theme::ThemeColors,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let hover = colors.element_hover;
    div()
        .id(id.into())
        .flex()
        .items_center()
        .px_3()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
        .on_click(on_click)
}
