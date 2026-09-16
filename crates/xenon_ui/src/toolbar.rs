//! The top toolbar: navigation, split actions, contextual status, quick actions.

use gpui::{
    Action, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::app::XenonApp;
use crate::chrome::list_selection;
use crate::icons::icon;
use crate::{
    CloseWorkspace, Copy, CopyClean, Cut, FilePalette, GoBack, GoForward, NewTerminal,
    NextWorkspace, Paste, PrevWorkspace, ReserveEmptyPaneRight, RunTask, Save, SplitDown,
    SplitRight, ToggleBrowser, ToggleSettings, ToggleSidebar,
};

const ICON: f32 = 16.;

impl XenonApp {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .h(px(44.))
            .px_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.toolbar_background)
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
                    glyph: Icon::List,
                    label: "Workspace Sidebar · ⌘B",
                    active: self.sidebar_visible(),
                    muted: false,
                    primary: false,
                    action: Box::new(ToggleSidebar),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-prev-workspace",
                    glyph: Icon::ArrowUp,
                    label: "Previous Workspace · ⌘⌥↑",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(PrevWorkspace),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-next-workspace",
                    glyph: Icon::ArrowDown,
                    label: "Next Workspace · ⌘⌥↓",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(NextWorkspace),
                },
                cx,
            ))
            .child(toolbar_divider(cx))
            .child(tool_button(
                ToolButton {
                    id: "tb-go-back",
                    glyph: Icon::ArrowLeft,
                    label: "Go Back · ⌘[",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(GoBack),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-go-forward",
                    glyph: Icon::ArrowRight,
                    label: "Go Forward · ⌘]",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(GoForward),
                },
                cx,
            ))
            .child(self.toolbar_layout_buttons(cx))
            .child(self.edit_tools(cx))
    }

    fn toolbar_layout_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(tool_button(
                ToolButton {
                    id: "tb-split-right",
                    // Reversed vs earlier assign: Horizontal glyph = left|right panes.
                    glyph: Icon::SplitSquareHorizontal,
                    label: "Split Right · ⌘\\",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(SplitRight),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-split-down",
                    glyph: Icon::SplitSquareVertical,
                    label: "Split Down · ⌘⇧\\",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(SplitDown),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-reserve-panel-right",
                    glyph: Icon::PanelRightDashed,
                    label: "Reserve Panel Right · ⌘⌥\\",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(ReserveEmptyPaneRight),
                },
                cx,
            ))
    }

    fn toolbar_center(&self, _cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div().flex_1().min_w_0()
    }

    fn toolbar_right(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dirty = self
            .active_editor()
            .is_some_and(|view| view.read(cx).is_dirty());
        let mut row = div().flex().items_center().gap_1();
        row = row.child(tool_button(
            ToolButton {
                id: "tb-new-terminal",
                glyph: Icon::SquareTerminal,
                label: "New Terminal · ⌘N",
                active: false,
                muted: false,
                primary: false,
                action: Box::new(NewTerminal),
            },
            cx,
        ));
        row = row.child(tool_button(
            ToolButton {
                id: "tb-palette",
                glyph: Icon::Search,
                label: "Go to File · ⌘P",
                active: false,
                muted: false,
                primary: false,
                action: Box::new(FilePalette),
            },
            cx,
        ));
        row = row.child(tool_button(
            ToolButton {
                id: "tb-close-workspace",
                glyph: Icon::X,
                label: "Close Workspace · ⌘⌥W",
                active: false,
                muted: false,
                primary: false,
                action: Box::new(CloseWorkspace),
            },
            cx,
        ));
        row = row.child(tool_button(
            ToolButton {
                id: "tb-file-browser",
                glyph: Icon::FolderTree,
                label: "File Browser · ⌘E",
                active: false,
                muted: false,
                primary: false,
                action: Box::new(ToggleBrowser),
            },
            cx,
        ));
        row = row.child(tool_button(
            ToolButton {
                id: "tb-run-task",
                glyph: Icon::Play,
                label: "Run Task · ⌘⇧R",
                active: false,
                muted: false,
                primary: true,
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
                    muted: false,
                    primary: false,
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
                muted: false,
                primary: false,
                action: Box::new(ToggleSettings),
            },
            cx,
        ))
    }

    fn edit_tools(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(toolbar_divider(cx))
            .child(tool_button(
                ToolButton {
                    id: "tb-cut",
                    glyph: Icon::Scissors,
                    label: "Cut · ⌘X",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(Cut),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-copy",
                    glyph: Icon::Copy,
                    label: "Copy · ⌘C",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(Copy),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-copy-clean",
                    glyph: Icon::CopyCheck,
                    label: "Copy Clean · ⌘⇧C",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(CopyClean),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-paste",
                    glyph: Icon::ClipboardPaste,
                    label: "Paste · ⌘V",
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(Paste),
                },
                cx,
            ))
    }
}

fn toolbar_divider(cx: &mut Context<XenonApp>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div().w(px(1.)).h(px(16.)).mx_1().bg(colors.border)
}

struct ToolButton {
    id: &'static str,
    glyph: Icon,
    label: &'static str,
    active: bool,
    muted: bool,
    primary: bool,
    action: Box<dyn Action>,
}

fn tool_button(button: ToolButton, cx: &mut Context<XenonApp>) -> impl IntoElement + use<> {
    let ToolButton {
        id,
        glyph,
        label,
        active,
        muted,
        primary,
        action: boxed,
    } = button;
    let colors = cx.theme().colors().clone();
    let highlighted = active || primary;
    let paint = list_selection(&colors, highlighted);
    let background = if highlighted {
        paint.background
    } else {
        gpui::transparent_black()
    };
    let fg = if highlighted {
        paint.foreground
    } else if muted {
        colors.text_muted
    } else {
        colors.icon
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
        .text_color(fg)
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
