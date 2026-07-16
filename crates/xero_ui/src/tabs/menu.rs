//! Keyboard-aware move-tab context menu.

use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px,
};
use theme::ActiveTheme;

use crate::app::{TabContextMenu, TabMove, TabSurface, XeroApp};

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
            selected: 0,
        });
        cx.stop_propagation();
        cx.notify();
    }

    /// Arrow/Enter/Esc for the move-tab menu when open from the keyboard.
    pub(crate) fn on_tab_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.tab_menu.clone() else {
            return false;
        };
        let siblings = self.sibling_streams(menu.stream);
        let len = 1 + siblings.len(); // new stream + siblings
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_tab_menu(cx);
                true
            }
            "up" => {
                if let Some(m) = self.tab_menu.as_mut() {
                    m.selected = m.selected.saturating_sub(1);
                }
                cx.notify();
                true
            }
            "down" => {
                if let Some(m) = self.tab_menu.as_mut() {
                    m.selected = (m.selected + 1).min(len.saturating_sub(1));
                }
                cx.notify();
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
        let siblings = self.sibling_streams(menu.stream);
        if menu.selected == 0 {
            match menu.surface {
                TabSurface::Terminal => {
                    self.move_terminal_tab_to_new_stream(menu.stream, menu.index, window, cx)
                }
                TabSurface::Editor => {
                    self.move_editor_tab_to_new_stream(menu.stream, menu.index, window, cx)
                }
            }
            return;
        }
        let Some((target, _)) = siblings.get(menu.selected - 1) else {
            return;
        };
        let tab = TabMove {
            from: menu.stream,
            index: menu.index,
            to: *target,
        };
        match menu.surface {
            TabSurface::Terminal => self.move_terminal_tab(tab, window, cx),
            TabSurface::Editor => self.move_editor_tab(tab, window, cx),
        }
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

        let selected = menu.selected;
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
                selected == 0,
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

        for (i, (target, name)) in siblings.into_iter().enumerate() {
            let label = format!("Move to {name}");
            let id = format!("tab-move-{target}");
            let row = i + 1;
            menu_box = menu_box.child(menu_item(
                id,
                label,
                &colors,
                selected == row,
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
