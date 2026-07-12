//! The top toolbar: panel toggles, context breadcrumb, and quick actions.

use gpui::{
    Action, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::app::XeroApp;
use crate::chrome::list_selection;
use crate::icons::icon;
use crate::{
    FilePalette, RunTask, Save, ToggleBrowser, ToggleEditor, ToggleSettings, ToggleSidebar,
    ToggleTerminal,
};

const ICON: f32 = 14.;

impl XeroApp {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .h(px(36.))
            .px_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.toolbar_left(cx))
            .child(self.toolbar_center(cx))
            .child(self.toolbar_right(cx))
    }

    fn toolbar_left(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(tool_button(
                ToolButton {
                    id: "tb-sidebar",
                    glyph: Icon::PanelLeft,
                    label: "Workspace Sidebar · ⌘B",
                    active: self.sidebar_visible(),
                    action: Box::new(ToggleSidebar),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-terminal",
                    glyph: Icon::Terminal,
                    label: "Terminal Panel",
                    active: self.terminal_visible(),
                    action: Box::new(ToggleTerminal),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-editor",
                    glyph: Icon::FileCode,
                    label: "Editor Panel",
                    active: self.editor_visible(),
                    action: Box::new(ToggleEditor),
                },
                cx,
            ))
            .child(toolbar_sep(cx))
            .child(tool_button(
                ToolButton {
                    id: "tb-tree",
                    glyph: Icon::FolderTree,
                    label: "File Tree · ⌘E",
                    active: self.is_browsing(),
                    action: Box::new(ToggleBrowser),
                },
                cx,
            ))
    }

    fn toolbar_center(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let label = self.breadcrumb_label().unwrap_or_default();
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .px_3()
            .text_sm()
            .text_color(colors.text_muted)
            .child(div().min_w_0().truncate().child(label))
    }

    fn toolbar_right(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dirty = self
            .active_editor()
            .is_some_and(|view| view.read(cx).is_dirty());
        let mut row = div()
            .flex()
            .items_center()
            .gap_1()
            .child(tool_button(
                ToolButton {
                    id: "tb-palette",
                    glyph: Icon::Search,
                    label: "Go to File · ⌘P",
                    active: false,
                    action: Box::new(FilePalette),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-run-task",
                    glyph: Icon::Play,
                    label: "Run Task · ⌘⇧R",
                    active: false,
                    action: Box::new(RunTask),
                },
                cx,
            ));
        if dirty {
            row = row.child(tool_button(
                ToolButton {
                    id: "tb-save",
                    glyph: Icon::Save,
                    label: "Save · ⌘S",
                    active: false,
                    action: Box::new(Save),
                },
                cx,
            ));
        }
        row.child(tool_button(
            ToolButton {
                id: "tb-settings",
                glyph: Icon::Settings,
                label: "Settings · ⌘,",
                active: false,
                action: Box::new(ToggleSettings),
            },
            cx,
        ))
    }
}

struct ToolButton {
    id: &'static str,
    glyph: Icon,
    label: &'static str,
    active: bool,
    action: Box<dyn Action>,
}

fn tool_button(button: ToolButton, cx: &mut Context<XeroApp>) -> impl IntoElement + use<> {
    let ToolButton {
        id,
        glyph,
        label,
        active,
        action: boxed,
    } = button;
    let colors = cx.theme().colors().clone();
    let paint = list_selection(&colors, active);
    // Inactive toggles sit on the toolbar, not on transparent (avoids hole).
    let background = if active {
        paint.background
    } else {
        colors.panel_background
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.))
        .h(px(26.))
        .rounded_sm()
        .bg(background)
        .text_color(paint.foreground)
        .cursor_pointer()
        .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
        .child(icon(glyph, px(ICON)))
        .tooltip({
            let text = gpui::SharedString::from(label);
            move |_window: &mut Window, cx: &mut gpui::App| {
                cx.new(|_| ToolbarTooltip { text: text.clone() }).into()
            }
        })
        .on_click(move |_, window: &mut Window, cx| {
            window.dispatch_action(boxed.boxed_clone(), cx);
        })
}

fn toolbar_sep(cx: &mut Context<XeroApp>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div().w(px(1.)).h(px(16.)).mx_1().bg(colors.border)
}

struct ToolbarTooltip {
    text: gpui::SharedString,
}

impl gpui::Render for ToolbarTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}
