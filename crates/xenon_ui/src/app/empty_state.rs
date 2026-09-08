//! Empty content state: explain the next useful action without a tour.

use gpui::{
    AnyElement, Context, Image, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, img, px,
};
use lucide_icons::Icon;
use std::sync::Arc;

use crate::commands::{CommandEntry, CommandId, catalog};
use xenon_core::{PaneId, SplitAxis};

use super::XenonApp;

fn xenon_icon() -> Arc<Image> {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../../../macos/xenon-icon-source.png").to_vec(),
    ))
}

impl XenonApp {
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_empty_state(
        &self,
        colors: theme::ThemeColors,
        remove_pane: Option<PaneId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let has_workspace = self.active.is_some();
        let title = if has_workspace {
            "Your workspace is ready"
        } else {
            "Open a workspace to get started"
        };
        let subtitle = if has_workspace {
            "Start a terminal or open a file. Your surfaces will appear here."
        } else {
            "Xenon keeps your agent work organized across workspaces."
        };
        let primary = if has_workspace {
            let hover = colors.element_hover;
            div()
                .id("empty-new-terminal")
                .px_3()
                .py_2()
                .rounded_sm()
                .bg(colors.element_selected)
                .text_color(colors.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child("New Terminal  ⌘N")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.new_terminal(window, cx);
                }))
                .into_any_element()
        } else {
            let hover = colors.element_hover;
            div()
                .id("empty-open-workspace")
                .px_3()
                .py_2()
                .rounded_sm()
                .bg(colors.element_selected)
                .text_color(colors.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child("Open Workspace  ⌘⇧O")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.add_workspace(window, cx);
                }))
                .into_any_element()
        };
        let secondary = has_workspace.then(|| {
            let hover = colors.element_hover;
            let text = colors.text;
            div()
                .id("empty-open-file")
                .px_3()
                .py_2()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(hover).text_color(text))
                .child("Open File  ⌘P")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_palette(window, cx);
                }))
                .into_any_element()
        });
        let reserve = has_workspace.then(|| {
            let hover = colors.element_hover;
            let text = colors.text;
            div()
                .id("empty-reserve-pane")
                .px_3()
                .py_2()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(hover).text_color(text))
                .child("Reserve Pane Right  ⌘⌥\\")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.park_empty_pane(SplitAxis::Horizontal, window, cx);
                }))
                .into_any_element()
        });
        let close_workspace = has_workspace.then(|| {
            let hover = colors.element_hover;
            let text = colors.text;
            div()
                .id("empty-close-workspace")
                .px_3()
                .py_2()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(hover).text_color(text))
                .child("Close Workspace  ⌘⌥W")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.close_active_workspace(window, cx);
                }))
                .into_any_element()
        });
        let remove_empty_pane = remove_pane.filter(|pane| {
            self.active_content()
                .and_then(|content| content.root.as_ref())
                .is_some_and(|root| {
                    root.leaf_ids().len() > 1
                        && root
                            .find_leaf(*pane)
                            .is_some_and(|leaf| leaf.tabs.is_empty())
                })
        });
        let remove_empty_pane = remove_empty_pane.map(|pane| {
            let hover = colors.element_hover;
            let muted = colors.text_muted;
            div()
                .id(("remove-empty-pane", pane.0))
                .absolute()
                .top_2()
                .right_2()
                .flex()
                .items_center()
                .justify_center()
                .w(px(24.))
                .h(px(24.))
                .rounded_sm()
                .text_color(muted)
                .cursor_pointer()
                .hover(move |s| s.bg(hover).text_color(colors.text))
                .tooltip(crate::tabs::tip_tooltip("Remove empty pane".into()))
                .child(crate::icons::icon(Icon::X, px(14.)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.remove_empty_pane(pane, window, cx);
                }))
                .into_any_element()
        });
        let shortcuts = shortcut_entries(has_workspace)
            .iter()
            .filter_map(|id| catalog().iter().find(|entry| entry.id == *id))
            .copied()
            .collect();
        empty_state_content(EmptyStateContent {
            colors,
            title,
            subtitle,
            primary,
            secondary,
            reserve,
            close_workspace,
            remove_empty_pane,
            shortcuts,
        })
    }
}

fn shortcut_entries(has_workspace: bool) -> &'static [CommandId] {
    if has_workspace {
        &[
            CommandId::CommandPalette,
            CommandId::FocusTerminal,
            CommandId::FocusEditor,
            CommandId::ToggleSidebar,
            CommandId::KeyboardHelp,
        ]
    } else {
        &[
            CommandId::CommandPalette,
            CommandId::ToggleSidebar,
            CommandId::KeyboardHelp,
        ]
    }
}

struct EmptyStateContent {
    colors: theme::ThemeColors,
    title: &'static str,
    subtitle: &'static str,
    primary: AnyElement,
    secondary: Option<AnyElement>,
    reserve: Option<AnyElement>,
    close_workspace: Option<AnyElement>,
    remove_empty_pane: Option<AnyElement>,
    shortcuts: Vec<CommandEntry>,
}

fn empty_state_content(content: EmptyStateContent) -> impl IntoElement {
    let EmptyStateContent {
        colors,
        title,
        subtitle,
        primary,
        secondary,
        reserve,
        close_workspace,
        remove_empty_pane,
        shortcuts,
    } = content;
    div()
        .relative()
        .flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .size_full()
        .children(remove_empty_pane)
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .max_w(px(420.))
                .text_center()
                .child(img(xenon_icon()).size(px(72.)))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(colors.text)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.text_muted)
                        .child(subtitle),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .pt_2()
                        .child(primary)
                        .children(secondary)
                        .children(reserve)
                        .children(close_workspace),
                )
                .child(
                    div()
                        .pt_3()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child("Useful shortcuts"),
                        )
                        .children(shortcuts.into_iter().map(|entry| {
                            div()
                                .flex()
                                .justify_between()
                                .gap_4()
                                .text_xs()
                                .child(div().text_color(colors.text_muted).child(entry.label))
                                .child(
                                    div()
                                        .text_color(colors.text)
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(entry.keys),
                                )
                        })),
                ),
        )
}
