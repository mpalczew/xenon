//! Keyboard-aware tab context menu (close only).

use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px,
};
use theme::ActiveTheme;
use xenon_core::{PaneId, TabId};

use crate::app::{TabContextMenu, XenonApp};

impl XenonApp {
    pub(crate) fn open_tab_menu(
        &mut self,
        _pane: PaneId,
        tab: TabId,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.active_workspace().is_none() {
            return;
        }
        self.tab_menu = Some(TabContextMenu { tab, position });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn on_tab_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tab_menu.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_tab_menu(cx);
                true
            }
            "enter" => {
                self.confirm_tab_menu(window, cx);
                true
            }
            _ => false,
        }
    }

    fn confirm_tab_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.tab_menu.take() else {
            return;
        };
        self.close_tab_id(menu.tab, window, cx);
    }

    pub(crate) fn dismiss_tab_menu(&mut self, cx: &mut Context<Self>) {
        if self.tab_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn render_tab_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let menu = self.tab_menu.as_ref()?;
        let colors = cx.theme().colors().clone();
        let tab = menu.tab;
        let position = menu.position;

        let menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(160.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(menu_item(
                "tab-close",
                "Close Tab",
                &colors,
                true,
                cx.listener(move |this, _, window, cx| {
                    this.dismiss_tab_menu(cx);
                    this.close_tab_id(tab, window, cx);
                }),
            ));

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
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let hover = colors.element_hover;
    let bg = if selected {
        colors.element_selected
    } else {
        gpui::transparent_black()
    };
    div()
        .id(id.into())
        .flex()
        .items_center()
        .px_3()
        .py_1()
        .text_sm()
        .bg(bg)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
        .on_click(on_click)
}
