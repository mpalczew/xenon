//! Mixed terminal/editor tab strips per leaf pane.

pub(crate) mod menu;
mod tooltips;

use gpui::{
    App, AppContext, Context, Focusable, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::PaneId;

use crate::{
    app::{DragTab, LiveLeaf, LiveTab, WorkspaceDot, XenonApp, workspace_dot},
    chrome::{self, SelectionPaint},
    icons::icon,
    preview_icon,
};
use tooltips::{DragGhost, TabTooltip};

fn tab_underline(paint: SelectionPaint) -> impl IntoElement {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(paint.accent)
}

fn tab_status_rail(status: WorkspaceDot, cx: &App) -> gpui::AnyElement {
    let color = match status {
        WorkspaceDot::Working => crate::chrome::working_color(cx),
        WorkspaceDot::Attention(_) => crate::chrome::attention_color(cx),
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(color)
        .into_any_element()
}

fn term_chip_paint(
    colors: &theme::ThemeColors,
    is_active: bool,
    is_focused: bool,
    is_exited: bool,
    cx: &App,
) -> chrome::SelectionPaint {
    let paint = chrome::tab_selection(colors, is_active, is_focused);
    if !is_exited {
        return paint;
    }
    let status = cx.theme().status();
    chrome::SelectionPaint {
        background: status.ignored_background,
        foreground: status.ignored,
        accent: status.ignored_border,
    }
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
        .tooltip(tip_tooltip(SharedString::from("Close Tab · ⌘W")))
        .child(icon(Icon::X, px(12.)))
        .on_click(on_click)
}

pub(crate) fn tip_tooltip(tip: SharedString) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    move |_window: &mut Window, cx: &mut App| cx.new(|_| TabTooltip { text: tip.clone() }).into()
}

impl XenonApp {
    pub(crate) fn render_mixed_tabs(
        &self,
        leaf: &LiveLeaf,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let pane = leaf.id;
        let chips = self.mixed_tab_chips(leaf, focused, cx);
        let preview = self.md_preview_btn(leaf, pane, &colors, cx);
        // Pin trailing chrome (+, md preview). Chips absorb width pressure so
        // those controls stay visible when many tabs are open.
        div()
            .flex()
            .items_center()
            .h(px(30.))
            .border_b_1()
            .border_color(colors.border)
            .bg(chrome::accent_surface(
                chrome::tab_bar_background(&colors),
                colors.text_accent,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap_1()
                    .px_1()
                    .overflow_hidden()
                    .children(chips),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_none()
                    .h_full()
                    .child(self.term_add_btn(pane, &colors, cx))
                    .children(preview),
            )
    }

    fn mixed_tab_chips(
        &self,
        leaf: &LiveLeaf,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let pane = leaf.id;
        let active = leaf.active;
        let ws = self.active;
        let mut chips = Vec::new();
        for (index, tab) in leaf.tabs.iter().enumerate() {
            let is_active = index == active;
            match tab {
                LiveTab::Terminal { id, view } => {
                    let term = view.read(cx);
                    let title = term.title(cx);
                    let exited = term.is_exited();
                    let tab_id = *id;
                    let attention = ws.and_then(|id| self.tab_attention(id, tab_id));
                    let status = workspace_dot(term.is_working(), attention);
                    chips.push(
                        self.mixed_term_chip(
                            pane,
                            index,
                            tab_id,
                            &title,
                            is_active,
                            focused && is_active,
                            exited,
                            status,
                            ws,
                            cx,
                        )
                        .into_any_element(),
                    );
                }
                LiveTab::Editor {
                    id,
                    path,
                    name,
                    view,
                } => {
                    let dirty = view.read(cx).is_dirty();
                    let tab_id = *id;
                    let path_s = path.display().to_string();
                    chips.push(
                        self.mixed_editor_chip(
                            pane,
                            index,
                            tab_id,
                            name,
                            &path_s,
                            is_active,
                            focused && is_active,
                            dirty,
                            ws,
                            cx,
                        )
                        .into_any_element(),
                    );
                }
            }
        }
        chips
    }

    fn md_preview_btn(
        &self,
        leaf: &LiveLeaf,
        pane: PaneId,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let show = leaf
            .active_tab()
            .and_then(|t| t.as_editor())
            .is_some_and(|v| v.read(cx).is_markdown());
        if !show {
            return None;
        }
        let previewing = self.active_editor_is_previewing(cx);
        let tip = SharedString::from(if previewing {
            "Show Source · ⌘⇧V"
        } else {
            "Markdown Preview · ⌘⇧V"
        });
        let colors = colors.clone();
        Some(
            div()
                .id(("md-preview", pane.0))
                .w(px(30.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .text_color(if previewing {
                    colors.text
                } else {
                    colors.text_muted
                })
                .cursor_pointer()
                .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
                .tooltip(tip_tooltip(tip))
                .child(preview_icon(previewing))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx))),
        )
    }

    fn term_add_btn(
        &self,
        pane: PaneId,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = colors.clone();
        div()
            .id(("term-add", pane.0))
            .px_2()
            .h_full()
            .flex()
            .items_center()
            .flex_none()
            .text_sm()
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
            .tooltip(tip_tooltip(SharedString::from("New Terminal · ⌘N")))
            .child(icon(Icon::Plus, px(13.)))
            .on_click(cx.listener(move |this, _, window, cx| {
                if let Some(content) = this.active.and_then(|id| this.contents.get_mut(&id)) {
                    content.focused = Some(pane);
                }
                this.add_terminal(cx);
                if let Some(t) = this.active_terminal() {
                    t.read(cx).focus_handle(cx).focus(window, cx);
                }
            }))
    }

    #[allow(clippy::too_many_arguments)]
    fn mixed_term_chip(
        &self,
        pane: PaneId,
        index: usize,
        tab_id: xenon_core::TabId,
        title: &str,
        is_active: bool,
        is_focused: bool,
        is_exited: bool,
        status: Option<WorkspaceDot>,
        ws: Option<xenon_core::WorkspaceId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let paint = term_chip_paint(&colors, is_active, is_focused, is_exited, cx);
        let group = format!("tab-{}-{}", pane.0, index);
        let tip = if is_exited {
            format!("{title} — process exited")
        } else if let Some(dot) = status {
            format!("{title} — {}", dot.tooltip())
        } else {
            title.to_string()
        };
        let title_owned = title.to_string();
        let display_title = if matches!(title, "bash" | "zsh" | "fish") {
            format!("{title} · {}", index + 1)
        } else {
            title_owned.clone()
        };
        div()
            .id(SharedString::from(format!("tab-{}-{}", pane.0, index)))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .flex_none()
            .min_w_0()
            .bg(paint.background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_tab_in_pane(pane, index, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_tab_menu(pane, tab_id, event.position, cx);
                }),
            )
            .on_drag(
                DragTab {
                    workspace: ws.unwrap_or_default(),
                    tab: tab_id,
                },
                {
                    let label = title_owned.clone();
                    move |_drag, _, _, cx| {
                        cx.new(|_| DragGhost {
                            label: SharedString::from(label.clone()),
                        })
                    }
                },
            )
            .tooltip({
                let full = SharedString::from(tip);
                move |_window: &mut Window, cx: &mut App| {
                    cx.new(|_| TabTooltip { text: full.clone() }).into()
                }
            })
            .child(
                div()
                    .text_color(if is_focused {
                        paint.accent
                    } else {
                        paint.foreground
                    })
                    .child(icon(Icon::SquareTerminal, px(13.))),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_focused {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .text_color(paint.foreground)
                    .max_w(px(160.))
                    .min_w_0()
                    .truncate()
                    .child(display_title),
            )
            .children(status.map(|dot| dot.pip(cx)))
            .children(status.map(|dot| tab_status_rail(dot, cx)))
            .child(tab_close(
                SharedString::from(format!("tab-close-{}-{}", pane.0, index)),
                &group,
                &colors,
                {
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab_id(tab_id, window, cx);
                    })
                },
            ))
            .child(tab_underline(paint))
    }

    #[allow(clippy::too_many_arguments)]
    fn mixed_editor_chip(
        &self,
        pane: PaneId,
        index: usize,
        tab_id: xenon_core::TabId,
        name: &str,
        path: &str,
        is_active: bool,
        is_focused: bool,
        is_dirty: bool,
        ws: Option<xenon_core::WorkspaceId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let paint = chrome::tab_selection(&colors, is_active, is_focused);
        let group = format!("tab-{}-{}", pane.0, index);
        let tip = SharedString::from(path.to_string());
        let name_owned = name.to_string();
        div()
            .id(SharedString::from(format!("etab-{}-{}", pane.0, index)))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h_full()
            .flex_none()
            .min_w_0()
            .bg(paint.background)
            .text_color(paint.foreground)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_tab_in_pane(pane, index, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_tab_menu(pane, tab_id, event.position, cx);
                }),
            )
            .on_drag(
                DragTab {
                    workspace: ws.unwrap_or_default(),
                    tab: tab_id,
                },
                {
                    let label = name_owned.clone();
                    move |_drag, _, _, cx| {
                        cx.new(|_| DragGhost {
                            label: SharedString::from(label.clone()),
                        })
                    }
                },
            )
            .tooltip(move |_window: &mut Window, cx: &mut App| {
                cx.new(|_| TabTooltip { text: tip.clone() }).into()
            })
            .child(
                div()
                    .text_color(if is_focused {
                        paint.accent
                    } else {
                        paint.foreground
                    })
                    .child(icon(Icon::FileText, px(13.))),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(if is_focused {
                        gpui::FontWeight::MEDIUM
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .max_w(px(160.))
                    .min_w_0()
                    .truncate()
                    .child(name_owned),
            )
            .children(is_dirty.then(|| chrome::status_pip(paint.foreground, false)))
            .child(tab_close(
                SharedString::from(format!("etab-close-{}-{}", pane.0, index)),
                &group,
                &colors,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_tab_id(tab_id, window, cx);
                }),
            ))
            .child(tab_underline(paint))
    }
}
