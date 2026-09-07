//! The collapsible left column: expandable Workspaces + Files sections.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::WorkspaceId;

use crate::app::{WorkspaceDot, XenonApp};
use crate::icons::icon;

mod widgets;

use widgets::{
    DragWorkspace, ICON_MD, ICON_SM, ROW_H, drag_chip, id_hash, path_tooltip,
    workspace_dirt_gutter, workspace_rename_row,
};

struct WorkspaceRows {
    id: WorkspaceId,
    name: String,
    path: String,
    active: bool,
    /// Workspace status pip (`None` = quiet).
    status: Option<WorkspaceDot>,
}

struct WorkspaceHeader<'a> {
    id: WorkspaceId,
    name: &'a str,
    path: &'a str,
    active: bool,
    status: Option<WorkspaceDot>,
    /// `+N` / `-M` line dirt when the workspace root is a dirty git work tree.
    dirt: Option<(String, String)>,
}

impl XenonApp {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let active = self.active_workspace();
        let workspaces: Vec<WorkspaceRows> = self
            .registry()
            .workspaces
            .iter()
            .map(|w| WorkspaceRows {
                id: w.id,
                name: w.name.clone(),
                path: w.root.display().to_string(),
                active: active == Some(w.id),
                status: self.workspace_status(w.id, cx),
            })
            .collect();

        let ws_collapsed = self.workspaces_collapsed();
        let files_open = self.is_browsing();
        let has_active = active.is_some();

        let mut ws_rows = Vec::new();
        if !ws_collapsed {
            for workspace in &workspaces {
                let dirt = self.workspace_dirt(workspace.id).and_then(|d| d.labels());
                ws_rows.push(
                    self.workspace_header(
                        WorkspaceHeader {
                            id: workspace.id,
                            name: &workspace.name,
                            path: &workspace.path,
                            active: workspace.active,
                            status: workspace.status,
                            dirt,
                        },
                        cx,
                    )
                    .into_any_element(),
                );
            }
            ws_rows.push(self.workspace_list_end_drop(cx).into_any_element());
        }

        // When Files is open: workspaces-first height (content or pinned), drag to resize.
        // When Files is closed: workspaces fill the column.
        let share_column = files_open && has_active;
        let body_h = share_column.then(|| {
            self.workspaces_section_height().unwrap_or_else(|| {
                crate::resize::workspaces_body_content_height(workspaces.len(), ROW_H)
            })
        });

        let workspace_body = (!ws_collapsed).then(|| {
            let rows = div()
                .id("sidebar-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .py_1()
                .children(ws_rows);
            div()
                .relative()
                .when(share_column, |s| {
                    s.flex_none()
                        .h(px(body_h.unwrap_or(crate::resize::MIN_WORKSPACES_BODY)))
                })
                .when(!share_column, |s| s.flex_1())
                .min_h_0()
                .flex()
                .flex_col()
                .child(rows)
                .when(share_column, |s| {
                    s.child(crate::resize::row_resize_handle(
                        "sidebar-section-resize",
                        crate::resize::ResizeEdge::SidebarSections,
                        colors.border,
                    ))
                })
        });

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
            .bg(crate::chrome::sidebar_background(&colors))
            .child(self.workspaces_section_header(ws_collapsed, cx))
            .children(workspace_body)
            .children(has_active.then(|| self.render_files_section(cx)))
            .child(crate::resize::col_resize_handle(
                "sidebar-resize",
                crate::resize::ResizeEdge::Sidebar,
                colors.border,
            ))
    }

    fn workspaces_section_header(
        &self,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let chevron = if collapsed {
            Icon::ChevronRight
        } else {
            Icon::ChevronDown
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(32.))
            .px_2()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .id("workspaces-toggle")
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors.text))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_workspaces_section(cx)))
                    .child(
                        div()
                            .w(px(14.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(colors.text_muted)
                            .child(icon(chevron, px(ICON_SM))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.text_muted)
                            .child("Workspaces"),
                    ),
            )
            .child(icon_button(
                "add-workspace",
                Icon::Plus,
                colors.clone(),
                Some("Open Workspace · ⌘⇧O"),
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.add_workspace_from_plus(window, cx);
                }),
            ))
    }

    fn workspace_header(
        &self,
        header: WorkspaceHeader,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let WorkspaceHeader {
            id,
            name,
            path,
            active,
            status,
            dirt,
        } = header;
        if let Some(field) = self.rename_workspace_field(id) {
            return workspace_rename_row(id, field, cx);
        }
        let colors = cx.theme().colors().clone();
        let group = format!("ws-{id}");
        let drop_line = colors.drop_target_border;
        let path_tip = SharedString::from(path.to_string());
        div()
            .id(("ws-row", id_hash(id.to_string())))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .h(px(ROW_H))
            .mx_1()
            .px_2()
            .rounded_sm()
            .text_sm()
            .when(active, |s| {
                s.bg(crate::chrome::accent_surface(
                    colors.element_selected,
                    colors.text_accent,
                ))
                .text_color(colors.text)
                .font_weight(gpui::FontWeight::MEDIUM)
            })
            .when(!active, |s| s.text_color(colors.text_muted))
            .border_l_2()
            .border_color(if active {
                colors.border_selected
            } else {
                gpui::transparent_black()
            })
            .tooltip(path_tooltip(path_tip))
            .on_drag(DragWorkspace(id), drag_chip(name))
            .can_drop(move |drag, _, _| {
                drag.downcast_ref::<DragWorkspace>()
                    .is_some_and(|d| d.0 != id)
            })
            .drag_over::<DragWorkspace>(move |style, _, _, _| style.border_color(drop_line))
            .on_drop(
                cx.listener(move |this, dragged: &DragWorkspace, _window, cx| {
                    this.reorder_workspace(dragged.0, id, cx)
                }),
            )
            .child(self.workspace_title_hit(id, name, status, cx))
            .child(workspace_dirt_gutter(dirt, &group, &colors))
            .child(self.workspace_hover_actions(id, &group, &colors, cx))
            .into_any_element()
    }

    /// Thin drop strip under the last open workspace (keeps gap small).
    fn workspace_list_end_drop(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let drop_line = colors.drop_target_border;
        div()
            .id("ws-drop-end")
            .h(px(4.))
            .mx_1()
            .border_t_2()
            .border_color(gpui::transparent_black())
            .can_drop(|drag, _, _| drag.downcast_ref::<DragWorkspace>().is_some())
            .drag_over::<DragWorkspace>(move |style, _, _, _| style.border_color(drop_line))
            .on_drop(cx.listener(|this, dragged: &DragWorkspace, _window, cx| {
                this.reorder_workspace_to_end(dragged.0, cx);
            }))
    }

    fn workspace_title_hit(
        &self,
        id: WorkspaceId,
        name: &str,
        status: Option<WorkspaceDot>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let active = self.active_workspace() == Some(id);
        let attention_dot = status.map(|dot| {
            div()
                .id(("ws-attention", id_hash(id.to_string())))
                .tooltip(path_tooltip(SharedString::from(dot.tooltip())))
                .child(dot.pip(cx))
        });
        div()
            .id(("ws-select", id_hash(id.to_string())))
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .flex_1()
            .cursor_pointer()
            .hover(|s| s.text_color(colors.text))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_workspace(id, window, cx);
            }))
            .child(
                div()
                    .text_color(if active {
                        colors.text_accent
                    } else {
                        colors.text_muted
                    })
                    .child(icon(Icon::Folder, px(ICON_SM))),
            )
            .child(div().truncate().child(name.to_string()))
            .children(attention_dot)
    }

    fn workspace_hover_actions(
        &self,
        id: WorkspaceId,
        group: &str,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let group = group.to_string();
        let active = self.active_workspace() == Some(id);
        let surface = if active {
            crate::chrome::accent_surface(colors.element_selected, colors.text_accent)
        } else {
            crate::chrome::sidebar_background(colors)
        };
        // Fixed equal slots so pencil / x share the same center grid as the gutter.
        div()
            .absolute()
            .right(px(2.))
            .top_0()
            .bottom_0()
            .w(px(widgets::ACTIONS_W))
            .flex()
            .items_center()
            .justify_end()
            .invisible()
            .group_hover(group, |s| s.visible())
            .bg(surface)
            .child(icon_button(
                ("ws-rename", id_hash(id.to_string())),
                Icon::Pencil,
                colors.clone(),
                Some("Rename Workspace"),
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.start_rename_workspace(id, cx);
                }),
            ))
            .child(icon_button(
                ("workspace-close", id_hash(id.to_string())),
                Icon::X,
                colors.clone(),
                Some("Close · ⌘⌥W"),
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_workspace(id, window, cx);
                }),
            ))
    }
}

pub(super) fn icon_button(
    id: impl Into<gpui::ElementId>,
    glyph: Icon,
    colors: theme::ThemeColors,
    tip: Option<&str>,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let tip = tip.map(SharedString::from);
    let danger = matches!(glyph, Icon::X);
    let danger_color = colors.version_control_deleted;
    let hover_background = if danger {
        danger_color.opacity(0.14)
    } else {
        colors.element_hover
    };
    let hover_foreground = if danger { danger_color } else { colors.text };
    // Equal hit targets; glyph centered in a fixed box so pencil/x share baseline.
    let mut btn = div()
        .id(id)
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(px(widgets::ACTION_BTN))
        .h(px(widgets::ACTION_BTN))
        .rounded_sm()
        .text_color(if danger {
            danger_color
        } else {
            colors.text_muted
        })
        .cursor_pointer()
        .hover(move |s| s.bg(hover_background).text_color(hover_foreground))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(ICON_MD))
                .child(icon(glyph, px(ICON_MD))),
        )
        .on_click(on_click);
    if danger && let Some(text) = tip.clone() {
        btn = btn.group("workspace-close-action").child(
            div()
                .absolute()
                .right_0()
                .bottom(px(24.))
                .invisible()
                .group_hover("workspace-close-action", |s| s.visible())
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(colors.elevated_surface_background)
                .border_1()
                .border_color(colors.border)
                .text_color(colors.text)
                .text_xs()
                .whitespace_nowrap()
                .child(text),
        );
    }
    match tip {
        Some(text) if !danger => btn.tooltip(path_tooltip(text)).into_any_element(),
        None => btn.into_any_element(),
        Some(_) => btn.into_any_element(),
    }
}
