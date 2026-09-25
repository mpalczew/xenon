//! The top toolbar: navigation, split actions, contextual status, quick actions.

use gpui::{
    Action, AppContext, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::app::XenonApp;
use crate::chrome::list_selection;
use crate::commands::{self, CommandId};
use crate::icons::icon;
use crate::{
    CaptureWorklist, CloseWorkspace, Copy, CopyClean, Cut, FilePalette, GoBack, GoForward,
    NewTerminal, NextWorkspace, OpenWorklist, Paste, PrevWorkspace, ReserveEmptyPaneRight, RunTask,
    Save, SplitDown, SplitRight, ToggleBrowser, ToggleSettings, ToggleSidebar,
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
                    command: CommandId::ToggleSidebar,
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
                    command: CommandId::PrevWorkspace,
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
                    command: CommandId::NextWorkspace,
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
                    command: CommandId::GoBack,
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
                    command: CommandId::GoForward,
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
                    command: CommandId::SplitRight,
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
                    command: CommandId::SplitDown,
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
                    command: CommandId::ReserveEmptyPaneRight,
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
                command: CommandId::NewTerminal,
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
                command: CommandId::GoToFile,
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
                command: CommandId::CloseWorkspace,
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
                command: CommandId::ToggleBrowser,
                active: false,
                muted: false,
                primary: false,
                action: Box::new(ToggleBrowser),
            },
            cx,
        ));
        row = row.child(self.worklist_buttons(cx));
        row = row.child(tool_button(
            ToolButton {
                id: "tb-run-task",
                glyph: Icon::Play,
                command: CommandId::RunTask,
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
                    command: CommandId::Save,
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
                command: CommandId::ToggleSettings,
                active: false,
                muted: false,
                primary: false,
                action: Box::new(ToggleSettings),
            },
            cx,
        ))
    }

    fn worklist_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(toolbar_divider(cx))
            .child(tool_button(
                ToolButton {
                    id: "tb-capture-worklist",
                    glyph: Icon::NotebookPen,
                    command: CommandId::CaptureWorklist,
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(CaptureWorklist),
                },
                cx,
            ))
            .child(tool_button(
                ToolButton {
                    id: "tb-open-worklist",
                    glyph: Icon::ListTodo,
                    command: CommandId::OpenWorklist,
                    active: false,
                    muted: false,
                    primary: false,
                    action: Box::new(OpenWorklist),
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
                    command: CommandId::Cut,
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
                    command: CommandId::Copy,
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
                    command: CommandId::CopyClean,
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
                    command: CommandId::Paste,
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
    command: CommandId,
    active: bool,
    muted: bool,
    primary: bool,
    action: Box<dyn Action>,
}

fn tool_button(button: ToolButton, cx: &mut Context<XenonApp>) -> impl IntoElement + use<> {
    let ToolButton {
        id,
        glyph,
        command,
        active,
        muted,
        primary,
        action: boxed,
    } = button;
    let colors = cx.theme().colors().clone();
    let command = commands::entry(command);
    let app = cx.entity();
    let command_id = command.id;
    let hint = if command.keys.is_empty() {
        command.label.to_string()
    } else {
        format!("{} · {}", command.label, command.keys)
    };
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
        .relative()
        .focusable()
        .tab_index(0)
        .focus_visible(|s| s.border_1().border_color(colors.border_focused))
        .rounded_sm()
        .bg(background)
        .text_color(fg)
        .aria_label(hint.clone())
        .cursor_pointer()
        .hover(move |s| s.bg(colors.element_hover).text_color(colors.text))
        .child(icon(glyph, px(ICON)))
        .child(toolbar_focus_hint(id, command, &colors))
        .tooltip({
            let label = gpui::SharedString::from(command.label);
            let keys = (!command.keys.is_empty()).then(|| gpui::SharedString::from(command.keys));
            move |_window: &mut Window, cx: &mut gpui::App| {
                cx.new(|_| ToolbarTooltip {
                    label: label.clone(),
                    keys: keys.clone(),
                })
                .into()
            }
        })
        .on_click(move |event, window: &mut Window, cx| {
            window.dispatch_action(boxed.boxed_clone(), cx);
            if matches!(event, ClickEvent::Mouse(_))
                && matches!(
                    command_id,
                    CommandId::Copy
                        | CommandId::Cut
                        | CommandId::Paste
                        | CommandId::CopyClean
                        | CommandId::Save
                )
            {
                app.update(cx, |app, cx| app.focus_after_teardown(Some(window), cx));
            }
        })
}

fn toolbar_focus_hint(
    id: &'static str,
    command: commands::CommandEntry,
    colors: &theme::ThemeColors,
) -> impl IntoElement + use<> {
    div()
        .id(format!("{id}-focus-hint"))
        .absolute()
        .top(px(30.))
        .right(px(0.))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .text_color(colors.text)
        .text_sm()
        .whitespace_nowrap()
        .opacity(0.)
        .focusable()
        .in_focus(|s| s.opacity(1.))
        .child(command.label)
        .children((!command.keys.is_empty()).then(|| {
            div()
                .px_1()
                .rounded_xs()
                .bg(colors.element_background)
                .text_color(colors.text_muted)
                .child(command.keys)
        }))
}

struct ToolbarTooltip {
    label: gpui::SharedString,
    keys: Option<gpui::SharedString>,
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
            .flex()
            .items_center()
            .gap_2()
            .child(self.label.clone())
            .children(self.keys.as_ref().map(|keys| {
                div()
                    .px_1()
                    .rounded_xs()
                    .bg(colors.element_background)
                    .text_color(colors.text_muted)
                    .child(keys.clone())
            }))
    }
}
