//! The collapsible left column: workspaces, each with its streams nested
//! beneath. Rendered as methods on `XeroApp` so click handlers use `cx.listener`.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use theme::ActiveTheme;
use xero_core::{StreamId, WorkspaceId};

use crate::app::XeroApp;

/// Owned view of one workspace and its streams, snapshotted so the registry
/// borrow is released before we reborrow `cx` per row.
struct WorkspaceRows {
    id: WorkspaceId,
    name: String,
    collapsed: bool,
    streams: Vec<(StreamId, String, bool)>,
}

impl XeroApp {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let active = self.active_stream();
        let workspaces: Vec<WorkspaceRows> = self
            .registry()
            .workspaces
            .iter()
            .map(|w| WorkspaceRows {
                id: w.id,
                name: w.name.clone(),
                collapsed: self.is_workspace_collapsed(w.id),
                streams: w
                    .streams
                    .iter()
                    .map(|&s| (s, self.stream_name(s).to_string(), active == Some(s)))
                    .collect(),
            })
            .collect();

        let mut rows = Vec::new();
        for workspace in workspaces {
            let header = self.workspace_header(workspace.id, &workspace.name, workspace.collapsed, cx);
            rows.push(header.into_any_element());
            if !workspace.collapsed {
                for (id, name, is_active) in workspace.streams {
                    rows.push(self.stream_row(id, &name, is_active, cx).into_any_element());
                }
            }
        }

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.sidebar_title(cx))
            .children(rows)
    }

    fn sidebar_title(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(div().text_sm().child("WORKSPACES"))
            .child(
                div()
                    .id("add-workspace")
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child("+")
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::AddWorkspace), cx);
                    })),
            )
    }

    fn workspace_header(
        &self,
        id: WorkspaceId,
        name: &str,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let chevron = if collapsed { "▸" } else { "▾" };
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .pt_2()
            .text_xs()
            .text_color(colors.text_muted)
            .child(
                div()
                    .id(("ws-toggle", id_hash(id.to_string())))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors.text))
                    .child(chevron)
                    .child(name.to_uppercase())
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_workspace(id, cx))),
            )
            .child(
                div()
                    .id(("add-stream", id_hash(id.to_string())))
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child("+")
                    .on_click(cx.listener(move |this, _, _, cx| this.add_stream(id, cx))),
            )
    }

    fn stream_row(
        &self,
        id: StreamId,
        name: &str,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let background =
            if is_active { colors.element_selected } else { colors.panel_background };
        // Amber dot when the stream's agent rang the bell while unfocused.
        let attention = self.needs_attention(id).then(|| {
            div().w(px(6.)).h(px(6.)).rounded_full().bg(gpui::rgb(0xd19a66))
        });
        div()
            .id(("stream", id_hash(id.to_string())))
            .flex()
            .items_center()
            .justify_between()
            .pl_5()
            .pr_2()
            .py_1()
            .text_sm()
            .bg(background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .on_click(cx.listener(move |this, _, window, cx| this.select_stream(id, window, cx)))
            .child(name.to_string())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(attention)
                    .child(
                        div()
                            .id(("stream-close", id_hash(id.to_string())))
                            .text_xs()
                            .text_color(colors.text_muted)
                            .hover(|s| s.text_color(colors.text))
                            .child("✕")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_stream(id, window, cx);
                            })),
                    ),
            )
    }
}

/// A stable element id from an id string.
fn id_hash(id: String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
