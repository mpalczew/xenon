//! Overflow count + tab list for a packed mixed-tab strip.

use gpui::{
    App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::{PaneId, TabId};

use crate::{
    app::{LiveLeaf, LiveTab, TabOverflowMenu, WorkspaceDot, XenonApp, workspace_dot},
    chrome,
    icons::icon,
    tabs::{
        layout::{OVERFLOW_BTN, PackedTabs, chip_width, pack_tabs},
        tip_tooltip,
    },
};

impl XenonApp {
    pub(crate) fn packed_tabs(&self, leaf: &LiveLeaf, cx: &App) -> PackedTabs {
        let widths: Vec<f32> = leaf
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| chip_width(&overflow_label(tab, i, cx)))
            .collect();
        let avail = self
            .tab_strip_widths
            .get(&leaf.id)
            .map(|width| f32::from(*width))
            .unwrap_or(f32::MAX);
        pack_tabs(&widths, leaf.active, avail)
    }

    pub(crate) fn toggle_overflow_menu(
        &mut self,
        pane: PaneId,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self
            .overflow_menu
            .as_ref()
            .is_some_and(|menu| menu.pane == pane)
        {
            self.dismiss_overflow_menu(cx);
            return;
        }
        let selected = self
            .active_content()
            .and_then(|c| c.root.as_ref()?.find_leaf(pane))
            .map(|leaf| leaf.active)
            .unwrap_or(0);
        self.tab_menu = None;
        self.browser_menu = None;
        self.overflow_menu = Some(TabOverflowMenu {
            pane,
            position,
            selected,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn dismiss_overflow_menu(&mut self, cx: &mut Context<Self>) {
        if self.overflow_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn on_overflow_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.overflow_menu.as_ref() else {
            return false;
        };
        let pane = menu.pane;
        let Some(len) = self.overflow_tab_count(pane) else {
            return false;
        };
        if len == 0 {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_overflow_menu(cx);
                true
            }
            "up" => {
                if let Some(m) = self.overflow_menu.as_mut() {
                    m.selected = m.selected.saturating_sub(1);
                    cx.notify();
                }
                true
            }
            "down" => {
                if let Some(m) = self.overflow_menu.as_mut() {
                    m.selected = (m.selected + 1).min(len.saturating_sub(1));
                    cx.notify();
                }
                true
            }
            "enter" => {
                let selected = menu.selected.min(len - 1);
                self.activate_tab_in_pane(pane, selected, window, cx);
                self.dismiss_overflow_menu(cx);
                true
            }
            "backspace" | "delete" => {
                let selected = menu.selected.min(len - 1);
                if let Some(tab) = self.overflow_tab_id(pane, selected) {
                    self.close_tab_id(tab, window, cx);
                    self.clamp_overflow_selected(pane, cx);
                }
                true
            }
            _ => false,
        }
    }

    fn overflow_tab_count(&self, pane: PaneId) -> Option<usize> {
        Some(
            self.active_content()?
                .root
                .as_ref()?
                .find_leaf(pane)?
                .tabs
                .len(),
        )
    }

    fn overflow_tab_id(&self, pane: PaneId, index: usize) -> Option<TabId> {
        Some(
            self.active_content()?
                .root
                .as_ref()?
                .find_leaf(pane)?
                .tabs
                .get(index)?
                .id(),
        )
    }

    fn clamp_overflow_selected(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        let Some(len) = self.overflow_tab_count(pane) else {
            self.dismiss_overflow_menu(cx);
            return;
        };
        if len == 0 {
            self.dismiss_overflow_menu(cx);
            return;
        }
        if let Some(menu) = self.overflow_menu.as_mut() {
            menu.selected = menu.selected.min(len - 1);
        }
        cx.notify();
    }

    pub(crate) fn render_overflow_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let menu = self.overflow_menu.as_ref()?;
        let pane = menu.pane;
        let selected = menu.selected;
        let position = menu.position;
        let leaf = self.active_content()?.root.as_ref()?.find_leaf(pane)?;
        let hidden = self.packed_tabs(leaf, cx).hidden;
        let hidden_ids: Vec<TabId> = hidden
            .iter()
            .filter_map(|&i| leaf.tabs.get(i).map(|t| t.id()))
            .collect();
        let colors = cx.theme().colors().clone();
        let mut list = div()
            .id("tab-overflow-list")
            .occlude()
            .flex()
            .flex_col()
            .w(px(280.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .py_1()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        for (index, tab) in leaf.tabs.iter().enumerate() {
            list = list.child(self.overflow_row(
                pane,
                index,
                tab,
                index == selected,
                hidden_ids.contains(&tab.id()),
                &colors,
                cx,
            ));
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.dismiss_overflow_menu(cx)),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| this.dismiss_overflow_menu(cx)),
                )
                .child(deferred(anchored().position(position).child(list)).with_priority(2)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn overflow_row(
        &self,
        pane: PaneId,
        index: usize,
        tab: &LiveTab,
        selected: bool,
        hidden: bool,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let paint = chrome::list_selection(colors, selected);
        let tab_id = tab.id();
        let title = overflow_label(tab, index, cx);
        let status = overflow_tab_status(self, tab, cx);
        let (bg, fg) = if selected {
            (paint.background, paint.foreground)
        } else if hidden {
            (colors.element_hover, colors.text_muted)
        } else {
            (gpui::transparent_black(), colors.text)
        };
        let mut row = div()
            .id(SharedString::from(format!(
                "overflow-row-{}-{index}",
                pane.0
            )))
            .flex()
            .items_center()
            .gap_2()
            .h(px(28.))
            .px_2()
            .cursor_pointer()
            .bg(bg)
            .text_color(fg)
            .font_weight(if selected {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if let Some(menu) = this.overflow_menu.as_mut()
                    && menu.selected != index
                {
                    menu.selected = index;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_tab_in_pane(pane, index, window, cx);
                this.dismiss_overflow_menu(cx);
            }))
            .child(overflow_row_pip(status, cx))
            .child(div().flex_1().min_w_0().text_sm().truncate().child(title));
        if selected {
            row = row.border_l_2().border_color(paint.accent);
        }
        row.child(
            div()
                .id(SharedString::from(format!(
                    "overflow-close-{}-{index}",
                    pane.0
                )))
                .w(px(22.))
                .h(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .text_color(colors.text_muted)
                .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                .tooltip(tip_tooltip(SharedString::from("Close")))
                .child(icon(Icon::X, px(12.)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_tab_id(tab_id, window, cx);
                    this.clamp_overflow_selected(pane, cx);
                })),
        )
    }
}

pub(crate) fn overflow_label(tab: &LiveTab, index: usize, cx: &App) -> String {
    match tab {
        LiveTab::Terminal { view, .. } => {
            let title = view.read(cx).title(cx);
            if matches!(title.as_str(), "bash" | "zsh" | "fish") {
                format!("{title} · {}", index + 1)
            } else {
                title
            }
        }
        LiveTab::Editor { name, .. } => name.clone(),
    }
}

pub(crate) fn hidden_status(
    app: &XenonApp,
    leaf: &LiveLeaf,
    hidden: &[usize],
    cx: &App,
) -> Option<WorkspaceDot> {
    let mut working = false;
    let mut attention = None;
    for &index in hidden {
        let Some(tab) = leaf.tabs.get(index) else {
            continue;
        };
        let Some(dot) = overflow_tab_status(app, tab, cx) else {
            continue;
        };
        match dot {
            WorkspaceDot::Working => working = true,
            WorkspaceDot::Attention(_) => attention = Some(dot),
        }
    }
    if working {
        Some(WorkspaceDot::Working)
    } else {
        attention
    }
}

fn overflow_tab_status(app: &XenonApp, tab: &LiveTab, cx: &App) -> Option<WorkspaceDot> {
    let LiveTab::Terminal { id, view } = tab else {
        return None;
    };
    let ws = app.active?;
    let attention = app.tab_attention(ws, *id);
    workspace_dot(view.read(cx).is_working(), attention)
}

fn overflow_row_pip(status: Option<WorkspaceDot>, cx: &App) -> gpui::AnyElement {
    let pip = div().w(px(6.)).h(px(6.)).rounded_full();
    match status {
        Some(dot) => overflow_status_pip(dot, false, cx),
        None => pip.into_any_element(),
    }
}

pub(crate) fn overflow_trigger(
    pane: PaneId,
    count: usize,
    open: bool,
    status: Option<WorkspaceDot>,
    cx: &mut gpui::Context<XenonApp>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let hover = colors.element_hover;
    let fg = if open { colors.text } else { colors.text_muted };
    let chevron = if open {
        Icon::ChevronUp
    } else {
        Icon::ChevronDown
    };
    div()
        .id(("tab-overflow", pane.0))
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .w(px(OVERFLOW_BTN))
        .h_full()
        .flex_none()
        .text_sm()
        .text_color(fg)
        .bg(if open {
            colors.element_hover
        } else {
            gpui::transparent_black()
        })
        .cursor_pointer()
        .hover(move |s| s.bg(hover).text_color(colors.text))
        .tooltip(tip_tooltip(SharedString::from(format!(
            "{count} more tabs"
        ))))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                this.toggle_overflow_menu(pane, event.position, cx);
            }),
        )
        .child(format!("{count}"))
        .child(icon(chevron, px(11.)))
        .children(status.map(|dot| overflow_status_pip(dot, true, cx)))
}

fn overflow_status_pip(status: WorkspaceDot, corner: bool, cx: &App) -> gpui::AnyElement {
    let attention = matches!(status, WorkspaceDot::Attention(_));
    let color = crate::chrome::status_color(cx, attention);
    let mut pip = div().w(px(5.)).h(px(5.)).rounded_full().bg(color);
    if corner {
        pip = pip.absolute().top(px(5.)).right(px(5.));
    }
    #[cfg(not(feature = "visual-tests"))]
    if matches!(status, WorkspaceDot::Working) {
        use gpui::AnimationExt;
        return pip
            .with_animation(
                "overflow-working-pip",
                gpui::Animation::new(std::time::Duration::from_millis(1400))
                    .repeat()
                    .with_easing(|delta| (delta * std::f32::consts::TAU).sin().mul_add(0.25, 0.75)),
                move |this, delta| this.opacity(delta),
            )
            .into_any_element();
    }
    pip.into_any_element()
}
