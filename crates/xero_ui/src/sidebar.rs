//! The collapsible left column: workspaces, each with its streams nested
//! beneath. Rendered as methods on `XeroApp` so click handlers use `cx.listener`.

use gpui::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;
use xero_core::{StreamId, WorkspaceId};

use crate::app::XeroApp;

/// Drag payloads, one type per kind so a stream can't drop onto the workspace
/// list and vice-versa.
struct DragStream(StreamId);
struct DragWorkspace(WorkspaceId);

/// The little label that follows the cursor while dragging a row.
struct DragChip {
    label: String,
}

impl Render for DragChip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .text_sm()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .child(self.label.clone())
    }
}

/// Owned view of one workspace and its streams, snapshotted so the registry
/// borrow is released before we reborrow `cx` per row.
struct WorkspaceRows {
    id: WorkspaceId,
    name: String,
    collapsed: bool,
    streams: Vec<(StreamId, String, bool)>,
}

struct ClosedWorkspaceRow {
    id: WorkspaceId,
    name: String,
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
        let closed_workspaces: Vec<ClosedWorkspaceRow> = self
            .registry()
            .closed_workspaces
            .iter()
            .map(|workspace| ClosedWorkspaceRow {
                id: workspace.id,
                name: workspace.name.clone(),
            })
            .collect();

        let mut rows = Vec::new();
        for workspace in workspaces {
            let header =
                self.workspace_header(workspace.id, &workspace.name, workspace.collapsed, cx);
            rows.push(header.into_any_element());
            if !workspace.collapsed {
                for (id, name, is_active) in workspace.streams {
                    rows.push(self.stream_row(id, &name, is_active, cx).into_any_element());
                }
            }
        }
        if !closed_workspaces.is_empty() {
            rows.push(self.closed_title(cx).into_any_element());
            for workspace in closed_workspaces {
                rows.push(
                    self.closed_workspace_row(workspace.id, &workspace.name, cx)
                        .into_any_element(),
                );
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

    fn closed_title(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .px_3()
            .pt_4()
            .pb_1()
            .text_xs()
            .text_color(colors.text_muted)
            .child("CLOSED")
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
            .id(("ws-row", id_hash(id.to_string())))
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .pt_2()
            .text_xs()
            .text_color(colors.text_muted)
            .on_drag(DragWorkspace(id), {
                let label = name.to_string();
                move |_, _, _, cx| {
                    cx.new(|_| DragChip {
                        label: label.clone(),
                    })
                }
            })
            .drag_over::<DragWorkspace>(move |style, _, _, _| style.bg(colors.element_selected))
            .on_drop(
                cx.listener(move |this, dragged: &DragWorkspace, _window, cx| {
                    this.reorder_workspace(dragged.0, id, cx)
                }),
            )
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
                    .flex()
                    .items_center()
                    .gap_1()
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
                    .child(
                        div()
                            .id(("workspace-close", id_hash(id.to_string())))
                            .px_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(colors.element_hover))
                            .child("✕")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_workspace(id, window, cx);
                            })),
                    ),
            )
    }

    fn closed_workspace_row(
        &self,
        id: WorkspaceId,
        name: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .id(("closed-workspace", id_hash(id.to_string())))
            .flex()
            .items_center()
            .justify_between()
            .pl_5()
            .pr_2()
            .py_1()
            .text_sm()
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .child(name.to_string())
            .child(
                div()
                    .id(("workspace-reopen", id_hash(id.to_string())))
                    .px_1()
                    .rounded_sm()
                    .child("↩"),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.reopen_workspace(id, cx)))
    }

    fn stream_row(
        &self,
        id: StreamId,
        name: &str,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let colors = cx.theme().colors().clone();
        let background = if is_active {
            colors.element_selected
        } else {
            colors.panel_background
        };
        // While renaming, the row is just the inline edit field.
        if let Some(field) = self.rename_field(id) {
            return div()
                .flex()
                .items_center()
                .pl_5()
                .pr_2()
                .py_1()
                .bg(background)
                .child(field)
                .into_any_element();
        }
        // Amber dot when the stream's agent rang the bell while unfocused.
        let attention = self.needs_attention(id).then(|| {
            div()
                .w(px(6.))
                .h(px(6.))
                .rounded_full()
                .bg(gpui::rgb(0xd19a66))
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
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        this.start_rename(id, cx);
                    } else {
                        this.select_stream(id, window, cx);
                    }
                }),
            )
            .on_drag(DragStream(id), {
                let label = name.to_string();
                move |_, _, _, cx| {
                    cx.new(|_| DragChip {
                        label: label.clone(),
                    })
                }
            })
            .drag_over::<DragStream>(move |style, _, _, _| style.bg(colors.element_selected))
            .on_drop(cx.listener(move |this, dragged: &DragStream, _window, cx| {
                this.reorder_stream(dragged.0, id, cx)
            }))
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
            .into_any_element()
    }
}

/// A stable element id from an id string.
fn id_hash(id: String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
