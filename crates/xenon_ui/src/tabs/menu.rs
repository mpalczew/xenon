//! Tab context menu (right-click / keyboard).

use gpui::{
    App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, px,
};
use theme::ActiveTheme;
use xenon_core::{PaneId, TabId};

use crate::app::{TabContextMenu, XenonApp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabMenuAction {
    Close,
    CloseOthers,
    CopyPath,
    CopyRelativePath,
    RevealInFinder,
    OpenInDefaultApp,
}

impl XenonApp {
    pub(crate) fn open_tab_menu(
        &mut self,
        pane: PaneId,
        tab: TabId,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.active_workspace().is_none() {
            return;
        }
        self.browser_menu = None;
        let _ = pane; // caller supplies pane for activate-before-menu if needed later
        self.tab_menu = Some(TabContextMenu {
            tab,
            position,
            selected: 0,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn on_tab_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.tab_menu.as_ref() else {
            return false;
        };
        let items = self.tab_menu_actions(menu.tab, cx);
        if items.is_empty() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_tab_menu(cx);
                true
            }
            "up" => {
                if let Some(m) = self.tab_menu.as_mut() {
                    m.selected = m.selected.saturating_sub(1);
                    cx.notify();
                }
                true
            }
            "down" => {
                if let Some(m) = self.tab_menu.as_mut() {
                    m.selected = (m.selected + 1).min(items.len().saturating_sub(1));
                    cx.notify();
                }
                true
            }
            "enter" => {
                let selected = menu.selected.min(items.len() - 1);
                let action = items[selected];
                let tab = menu.tab;
                self.dismiss_tab_menu(cx);
                self.run_tab_menu_action(action, tab, window, cx);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn dismiss_tab_menu(&mut self, cx: &mut Context<Self>) {
        if self.tab_menu.take().is_some() {
            cx.notify();
        }
    }

    fn tab_menu_actions(&self, tab: TabId, _cx: &App) -> Vec<TabMenuAction> {
        let mut items = vec![TabMenuAction::Close];
        if self.pane_tab_count(tab) > 1 {
            items.push(TabMenuAction::CloseOthers);
        }
        if self.tab_editor_path(tab).is_some() {
            items.extend([
                TabMenuAction::CopyPath,
                TabMenuAction::CopyRelativePath,
                TabMenuAction::RevealInFinder,
                TabMenuAction::OpenInDefaultApp,
            ]);
        }
        items
    }

    fn run_tab_menu_action(
        &mut self,
        action: TabMenuAction,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            TabMenuAction::Close => self.close_tab_id(tab, window, cx),
            TabMenuAction::CloseOthers => self.close_other_tabs(tab, window, cx),
            TabMenuAction::CopyPath => {
                if let Some(path) = self.tab_editor_path(tab) {
                    Self::copy_path_abs(&path, cx);
                }
            }
            TabMenuAction::CopyRelativePath => {
                if let Some(path) = self.tab_editor_path(tab) {
                    self.copy_path_relative(&path, cx);
                }
            }
            TabMenuAction::RevealInFinder => {
                if let Some(path) = self.tab_editor_path(tab) {
                    Self::reveal_in_finder(&path);
                }
            }
            TabMenuAction::OpenInDefaultApp => {
                if let Some(path) = self.tab_editor_path(tab) {
                    cx.open_with_system(&path);
                }
            }
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
        let selected = menu.selected;
        let items = self.tab_menu_actions(tab, cx);
        if items.is_empty() {
            return None;
        }

        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());

        for (i, action) in items.into_iter().enumerate() {
            let label = tab_menu_label(action);
            let is_sel = i == selected;
            menu_box = menu_box.child(menu_item(
                SharedString::from(format!("tab-menu-{i}")),
                label,
                &colors,
                is_sel,
                cx.listener(move |this, _, window, cx| {
                    this.dismiss_tab_menu(cx);
                    this.run_tab_menu_action(action, tab, window, cx);
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

fn tab_menu_label(action: TabMenuAction) -> &'static str {
    match action {
        TabMenuAction::Close => "Close Tab",
        TabMenuAction::CloseOthers => "Close Other Tabs",
        TabMenuAction::CopyPath => "Copy Path",
        TabMenuAction::CopyRelativePath => "Copy Relative Path",
        TabMenuAction::RevealInFinder => "Reveal in Finder",
        TabMenuAction::OpenInDefaultApp => "Open in Default App",
    }
}

pub(crate) fn menu_item(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    colors: &theme::ThemeColors,
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let hover = colors.element_selected;
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
