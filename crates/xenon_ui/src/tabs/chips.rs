//! Individual terminal/editor tab chips.

#[cfg(not(feature = "visual-tests"))]
use gpui::{Animation, AnimationExt};
use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::PaneId;

use super::tip_tooltip;
use super::tooltips::{DragGhost, TabTooltip};
use crate::{
    app::{DragTab, WorkspaceDot, XenonApp},
    chrome::{self, SelectionPaint},
    icons::icon,
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

fn tab_status_rail(status: WorkspaceDot, selected: bool, cx: &App) -> Option<gpui::AnyElement> {
    if selected {
        return None;
    }
    let color = match status {
        WorkspaceDot::Working => crate::chrome::status_color(cx, false),
        WorkspaceDot::Attention(_) => crate::chrome::status_color(cx, true),
    };
    let rail = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(color);
    #[cfg(not(feature = "visual-tests"))]
    if matches!(status, WorkspaceDot::Working) {
        return Some(
            rail.with_animation(
                "tab-working-rail",
                Animation::new(std::time::Duration::from_millis(1400))
                    .repeat()
                    .with_easing(|delta| (delta * std::f32::consts::TAU).sin().mul_add(0.25, 0.75)),
                move |this, delta| this.opacity(delta),
            )
            .into_any_element(),
        );
    }
    Some(rail.into_any_element())
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
    always: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let group = group.to_string();
    let hover = colors.text;
    let mut close = div()
        .id(id)
        .text_xs()
        .text_color(colors.text_muted)
        .hover(move |s| s.text_color(hover))
        .tooltip(tip_tooltip(SharedString::from("Close Tab · ⌘W")))
        .child(icon(Icon::X, px(12.)))
        .on_click(on_click);
    if !always {
        close = close.invisible().group_hover(group, |s| s.visible());
    }
    close
}

impl XenonApp {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn mixed_term_chip(
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
        let dragging = self.dragging_tab == Some(tab_id) && cx.has_active_drag();
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
            .border_r_1()
            .border_color(colors.border)
            .bg(paint.background)
            .opacity(if dragging { 0.4 } else { 1.0 })
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
                    let entity = cx.entity();
                    let ghost_paint = paint;
                    let ghost_focused = is_focused;
                    move |drag, _, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.dragging_tab = Some(drag.tab);
                            cx.notify();
                        });
                        cx.new(|_| DragGhost {
                            label: SharedString::from(label.clone()),
                            background: ghost_paint.background,
                            foreground: ghost_paint.foreground,
                            accent: ghost_paint.accent,
                            focused: ghost_focused,
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
            .children(status.and_then(|dot| tab_status_rail(dot, is_active, cx)))
            .child(tab_close(
                SharedString::from(format!("tab-close-{}-{}", pane.0, index)),
                &group,
                &colors,
                is_active,
                {
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab_id(tab_id, window, cx);
                    })
                },
            ))
            .child(tab_underline(paint))
            .children(super::XenonApp::tab_drop_slots(
                self, pane, index, tab_id, cx,
            ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn mixed_editor_chip(
        &self,
        pane: PaneId,
        index: usize,
        tab_id: xenon_core::TabId,
        name: &str,
        path: &str,
        is_active: bool,
        is_focused: bool,
        _is_dirty: bool,
        ws: Option<xenon_core::WorkspaceId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let paint = chrome::tab_selection(&colors, is_active, is_focused);
        let group = format!("tab-{}-{}", pane.0, index);
        let dragging = self.dragging_tab == Some(tab_id) && cx.has_active_drag();
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
            .border_r_1()
            .border_color(colors.border)
            .bg(paint.background)
            .opacity(if dragging { 0.4 } else { 1.0 })
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
                    let entity = cx.entity();
                    let ghost_paint = paint;
                    let ghost_focused = is_focused;
                    move |drag, _, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.dragging_tab = Some(drag.tab);
                            cx.notify();
                        });
                        cx.new(|_| DragGhost {
                            label: SharedString::from(label.clone()),
                            background: ghost_paint.background,
                            foreground: ghost_paint.foreground,
                            accent: ghost_paint.accent,
                            focused: ghost_focused,
                        })
                    }
                },
            )
            .tooltip(move |_window: &mut Window, cx: &mut App| {
                cx.new(|_| TabTooltip { text: tip.clone() }).into()
            })
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
            .child(tab_close(
                SharedString::from(format!("etab-close-{}-{}", pane.0, index)),
                &group,
                &colors,
                is_active,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_tab_id(tab_id, window, cx);
                }),
            ))
            .child(tab_underline(paint))
            .children(super::XenonApp::tab_drop_slots(
                self, pane, index, tab_id, cx,
            ))
    }
}
