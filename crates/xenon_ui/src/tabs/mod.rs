//! Mixed terminal/editor tab strips per leaf pane.

mod chips;
pub(crate) mod layout;
pub(crate) mod menu;
pub(crate) mod overflow;
mod tooltips;

use gpui::{
    App, AppContext, Context, Focusable, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder,
    px, relative,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::{PaneId, TabId};

use crate::{
    app::{DragTab, LiveLeaf, LiveTab, XenonApp, workspace_dot},
    icons::icon,
    preview_icon,
};
use tooltips::TabTooltip;

#[derive(Clone, Copy)]
enum TabDropSide {
    Before,
    After,
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
        let packed = self.packed_tabs(leaf, cx);
        let chips = self.mixed_tab_chips(leaf, &packed.visible, focused, cx);
        let wrap = self.wrap_lines_btn(leaf, pane, &colors, cx);
        let preview = self.md_preview_btn(leaf, pane, &colors, cx);
        let overflow_open = self
            .overflow_menu
            .as_ref()
            .is_some_and(|menu| menu.pane == pane);
        let overflow = (!packed.hidden.is_empty()).then(|| {
            overflow::overflow_trigger(
                pane,
                packed.hidden.len(),
                overflow_open,
                overflow::hidden_status(self, leaf, &packed.hidden, cx),
                cx,
            )
        });
        let entity = cx.entity();
        // Pin trailing chrome (+, wrap, md preview). Overflow count sits in the
        // chip row so + stays visible.
        div()
            .flex()
            .items_center()
            .h(px(34.))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.tab_bar_background)
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        canvas(
                            |bounds, _, _| bounds.size.width,
                            move |_bounds, width, _, cx| {
                                entity.update(cx, |this, cx| {
                                    let prev = this.tab_strip_widths.get(&pane).copied();
                                    let changed = prev.is_none_or(|old| {
                                        (f32::from(old) - f32::from(width)).abs() > 1.0
                                    });
                                    if changed {
                                        this.tab_strip_widths.insert(pane, width);
                                        cx.notify();
                                    }
                                });
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .gap_0()
                            .px_1()
                            .overflow_hidden()
                            .children(chips),
                    )
                    .children(overflow),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_none()
                    .h_full()
                    .child(self.term_add_btn(pane, &colors, cx))
                    .children(wrap)
                    .children(preview),
            )
    }

    fn mixed_tab_chips(
        &self,
        leaf: &LiveLeaf,
        visible: &[usize],
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let pane = leaf.id;
        let active = leaf.active;
        let ws = self.active;
        let mut chips = Vec::new();
        for &index in visible {
            let Some(tab) = leaf.tabs.get(index) else {
                continue;
            };
            let is_active = index == active;
            let chip = match tab {
                LiveTab::Terminal { id, view } => {
                    let term = view.read(cx);
                    let title = term.title(cx);
                    let exited = term.is_exited();
                    let tab_id = *id;
                    let attention = ws.and_then(|id| self.tab_attention(id, tab_id));
                    let status = workspace_dot(term.is_working(), attention);
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
                    .into_any_element()
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
                    .into_any_element()
                }
            };
            chips.push(chip);
        }
        chips
    }

    pub(super) fn tab_drop_slots(
        &self,
        pane: PaneId,
        index: usize,
        tab_id: TabId,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let workspace = self.active.unwrap_or_default();
        let entity = cx.entity();
        let slot = |side: TabDropSide| {
            let id = match side {
                TabDropSide::Before => format!("tab-drop-before-{}-{}", pane.0, index),
                TabDropSide::After => format!("tab-drop-after-{}-{}", pane.0, index),
            };
            let insert_at = match side {
                TabDropSide::Before => index,
                TabDropSide::After => index + 1,
            };
            let entity = entity.clone();
            div()
                .id(SharedString::from(id))
                .absolute()
                .top_0()
                .bottom_0()
                .w(relative(0.5))
                .when(matches!(side, TabDropSide::Before), |s| s.left_0())
                .when(matches!(side, TabDropSide::After), |s| s.right_0())
                .can_drop(move |drag, _, _| {
                    drag.downcast_ref::<DragTab>()
                        .is_some_and(|d| d.workspace == workspace && d.tab != tab_id)
                })
                .drag_over::<DragTab>(move |style, drag, window, cx| {
                    if drag.tab != tab_id {
                        entity.update(cx, |this, cx| {
                            this.move_tab_to_pane_at(drag.tab, pane, insert_at, window, cx);
                        });
                    }
                    style
                })
                .on_drop(cx.listener(move |this, drag: &DragTab, window, cx| {
                    if drag.workspace == workspace && drag.tab != tab_id {
                        this.dragging_tab = None;
                        this.move_tab_to_pane_at(drag.tab, pane, insert_at, window, cx);
                    }
                }))
        };

        vec![
            slot(TabDropSide::Before).into_any_element(),
            slot(TabDropSide::After).into_any_element(),
        ]
    }

    fn wrap_lines_btn(
        &self,
        leaf: &LiveLeaf,
        pane: PaneId,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let editor = leaf.active_tab().and_then(|tab| tab.as_editor())?;
        let path = {
            let view = editor.read(cx);
            if !view.is_text() {
                return None;
            }
            view.path().to_path_buf()
        };
        let wrapping = xenon_settings::editor_wraps(&path, cx);
        let colors = colors.clone();
        Some(
            xenon_design_system::action_button(
                ("wrap-lines", pane.0),
                xenon_design_system::ActionButton::icon(icon(Icon::TextWrap, px(15.))),
                cx,
                cx.listener(move |this, _, _, cx| {
                    this.toggle_soft_wrap_for(Some(&path), cx);
                }),
            )
            .w(px(30.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .text_color(if wrapping {
                colors.text
            } else {
                colors.text_muted
            })
            .cursor_pointer()
            .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
            .tooltip(tip_tooltip(SharedString::from("Wrap Lines · ⌥Z"))),
        )
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
            .is_some_and(|view| {
                matches!(
                    view.read(cx)
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("md" | "markdown" | "mdx")
                )
            });
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
            xenon_design_system::action_button(
                ("md-preview", pane.0),
                xenon_design_system::ActionButton::icon(preview_icon(previewing)),
                cx,
                cx.listener(|this, _, _, cx| this.toggle_preview(cx)),
            )
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
            .tooltip(tip_tooltip(tip)),
        )
    }

    fn term_add_btn(
        &self,
        pane: PaneId,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = colors.clone();
        xenon_design_system::action_button(
            ("term-add", pane.0),
            xenon_design_system::ActionButton::icon(icon(Icon::Plus, px(13.))),
            cx,
            cx.listener(move |this, _, window, cx| {
                if let Some(content) = this.active.and_then(|id| this.contents.get_mut(&id)) {
                    content.focused = Some(pane);
                }
                this.add_terminal(cx);
                if let Some(t) = this.active_terminal() {
                    t.read(cx).focus_handle(cx).focus(window, cx);
                }
            }),
        )
        .h_full()
        .flex_none()
        .text_color(colors.text_muted)
        .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
        .tooltip(tip_tooltip(SharedString::from("New Terminal · ⌘N")))
    }
}
