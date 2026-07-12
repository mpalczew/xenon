//! The collapsible left column: workspaces with nested streams.

use gpui::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px, transparent_black,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xero_core::{StreamId, WorkspaceId};

use crate::app::XeroApp;
use crate::icons::icon;

struct DragStream(StreamId);
struct DragWorkspace(WorkspaceId);

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

struct WorkspaceRows {
    id: WorkspaceId,
    name: String,
    collapsed: bool,
    streams: Vec<(StreamId, String, bool)>,
}

struct WorkspaceHeader<'a> {
    id: WorkspaceId,
    name: &'a str,
    collapsed: bool,
    /// `+N` / `-M` line dirt when the workspace root is a dirty git work tree.
    dirt: Option<(String, String)>,
}

struct StreamRow<'a> {
    id: StreamId,
    name: &'a str,
    is_active: bool,
}

const ROW_H: f32 = 24.;
const ICON_SM: f32 = 12.;
const ICON_MD: f32 = 13.;
const ATTENTION: u32 = 0xd19a66;

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
        let closed: Vec<(WorkspaceId, String)> = self
            .registry()
            .closed_workspaces
            .iter()
            .map(|w| (w.id, w.name.clone()))
            .collect();
        let closed_collapsed = self.closed_section_collapsed();

        let mut rows = Vec::new();
        for workspace in workspaces {
            let dirt = self.workspace_dirt(workspace.id).and_then(|d| d.labels());
            rows.push(
                self.workspace_header(
                    WorkspaceHeader {
                        id: workspace.id,
                        name: &workspace.name,
                        collapsed: workspace.collapsed,
                        dirt,
                    },
                    cx,
                )
                .into_any_element(),
            );
            if !workspace.collapsed {
                rows.push(self.stream_group(workspace.streams, cx).into_any_element());
            }
        }
        if !closed.is_empty() {
            rows.push(self.closed_title(closed_collapsed, cx).into_any_element());
            if !closed_collapsed {
                for (id, name) in closed {
                    rows.push(self.closed_workspace_row(id, &name, cx).into_any_element());
                }
            }
        }

        div()
            .relative()
            .flex()
            .flex_col()
            .w(px(self.sidebar_width_px()))
            .flex_none()
            .h_full()
            .min_h_0()
            .min_w_0()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.sidebar_title(cx))
            .child(
                div()
                    .id("sidebar-rows")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(rows),
            )
            .child(crate::resize::col_resize_handle(
                "sidebar-resize",
                crate::resize::ResizeEdge::Sidebar,
                colors.border,
            ))
    }

    fn sidebar_title(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(36.))
            .px_3()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.text_muted)
                    .child("Workspaces"),
            )
            .child(self.icon_button(
                "add-workspace",
                Icon::Plus,
                colors.clone(),
                cx.listener(|_, _, window, cx| {
                    window.dispatch_action(Box::new(crate::AddWorkspace), cx);
                }),
            ))
    }

    fn closed_title(&self, collapsed: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .id("closed-section")
            .flex()
            .items_center()
            .gap_1()
            .h(px(ROW_H))
            .mt_2()
            .mx_2()
            .px_1()
            .rounded_sm()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_closed_section(cx)))
            .child(chevron_slot(collapsed))
            .child("Closed")
    }

    fn workspace_header(
        &self,
        header: WorkspaceHeader,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let WorkspaceHeader {
            id,
            name,
            collapsed,
            dirt,
        } = header;
        let colors = cx.theme().colors().clone();
        let group = format!("ws-{id}");
        div()
            .id(("ws-row", id_hash(id.to_string())))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .h(px(ROW_H))
            .mx_1()
            .pl_2()
            .pr(px(2.))
            .rounded_sm()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.text_muted)
            .on_drag(DragWorkspace(id), drag_chip(name))
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
                    .min_w_0()
                    .flex_1()
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors.text))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_workspace(id, cx)))
                    .child(chevron_slot(collapsed))
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .child(icon(Icon::Folder, px(ICON_SM))),
                    )
                    .child(div().truncate().child(name.to_string())),
            )
            .child(
                div()
                    .absolute()
                    .right(px(2.))
                    .flex()
                    .items_center()
                    .gap_px()
                    .invisible()
                    .group_hover(group, |s| s.visible())
                    .bg(colors.panel_background)
                    .child(self.icon_button(
                        ("add-stream", id_hash(id.to_string())),
                        Icon::Plus,
                        colors.clone(),
                        cx.listener(move |this, _, _, cx| this.add_stream(id, cx)),
                    ))
                    .child(self.icon_button(
                        ("workspace-close", id_hash(id.to_string())),
                        Icon::X,
                        colors.clone(),
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_workspace(id, window, cx);
                        }),
                    )),
            )
            .children(dirt.map(|(plus, minus)| {
                crate::git_dirt::badge(
                    plus,
                    minus,
                    colors.version_control_added,
                    colors.version_control_deleted,
                )
            }))
    }

    /// Streams under a workspace, indented with a quiet vertical guide.
    fn stream_group(
        &self,
        streams: Vec<(StreamId, String, bool)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let rows: Vec<_> = streams
            .iter()
            .map(|(id, name, is_active)| {
                self.stream_row(
                    StreamRow {
                        id: *id,
                        name,
                        is_active: *is_active,
                    },
                    cx,
                )
                .into_any_element()
            })
            .collect();
        div()
            .flex()
            .pl(px(18.))
            .pr_1()
            .child(
                div()
                    .w(px(1.))
                    .flex_none()
                    .my_1()
                    .bg(colors.border)
                    .rounded_full(),
            )
            .child(div().flex_1().min_w_0().children(rows))
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
            .h(px(ROW_H))
            .ml(px(18.))
            .mr_1()
            .px_2()
            .rounded_sm()
            .text_sm()
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .on_click(cx.listener(move |this, _, _, cx| this.reopen_workspace(id, cx)))
            .child(div().truncate().child(name.to_string()))
    }

    fn stream_row(&self, row: StreamRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let StreamRow {
            id,
            name,
            is_active,
        } = row;
        let colors = cx.theme().colors().clone();
        let background = if is_active {
            colors.element_selected
        } else {
            transparent_black()
        };
        let accent = if is_active {
            colors.border_selected
        } else {
            transparent_black()
        };
        let group = format!("stream-{id}");
        if let Some(field) = self.rename_field(id) {
            return div()
                .flex()
                .items_center()
                .h(px(ROW_H))
                .px_1()
                .rounded_sm()
                .bg(background)
                .border_l_2()
                .border_color(accent)
                .child(field)
                .into_any_element();
        }
        let attention = self.needs_attention(id).then(|| {
            div()
                .w(px(6.))
                .h(px(6.))
                .rounded_full()
                .bg(gpui::rgb(ATTENTION))
        });
        div()
            .id(("stream", id_hash(id.to_string())))
            .group(group.clone())
            .flex()
            .items_center()
            .justify_between()
            .h(px(ROW_H))
            .px_1()
            .rounded_sm()
            .text_sm()
            .text_color(colors.text)
            .bg(background)
            .border_l_2()
            .border_color(accent)
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
            .on_drag(DragStream(id), drag_chip(name))
            .drag_over::<DragStream>(move |style, _, _, _| style.bg(colors.element_selected))
            .on_drop(cx.listener(move |this, dragged: &DragStream, _window, cx| {
                this.reorder_stream(dragged.0, id, cx)
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .pl_1()
                    .child(name.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .children(attention)
                    .child(div().invisible().group_hover(group, |s| s.visible()).child(
                        self.icon_button(
                            ("stream-close", id_hash(id.to_string())),
                            Icon::X,
                            colors.clone(),
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_stream(id, window, cx);
                            }),
                        ),
                    )),
            )
            .into_any_element()
    }

    fn icon_button(
        &self,
        id: impl Into<gpui::ElementId>,
        glyph: Icon,
        colors: theme::ThemeColors,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .w(px(20.))
            .h(px(20.))
            .rounded_sm()
            .text_color(colors.text_muted)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .child(icon(glyph, px(ICON_MD)))
            .on_click(on_click)
    }
}

fn chevron_slot(collapsed: bool) -> impl IntoElement {
    let glyph = if collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    div()
        .w(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(glyph, px(ICON_SM)))
}

fn drag_chip<T: 'static>(
    label: &str,
) -> impl Fn(&T, gpui::Point<gpui::Pixels>, &mut Window, &mut gpui::App) -> gpui::Entity<DragChip> + use<T>
{
    let label = label.to_string();
    move |_, _, _, cx| {
        cx.new(|_| DragChip {
            label: label.clone(),
        })
    }
}

fn id_hash(id: String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
