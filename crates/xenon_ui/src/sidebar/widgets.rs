//! Shared sidebar chrome: drag chips, tooltips, row helpers.

use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_core::WorkspaceId;

use crate::app::XenonApp;
use crate::icons::icon;

pub(super) struct DragWorkspace(pub WorkspaceId);

pub(super) struct DragChip {
    pub label: String,
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

/// Hover path (or other short text) for sidebar rows.
pub(super) struct PathTooltip {
    pub text: SharedString,
}

impl Render for PathTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}

pub(super) const ROW_H: f32 = 24.;
pub(super) const ICON_SM: f32 = 14.;
pub(super) const ICON_MD: f32 = 15.;
/// Square hit target for pencil / close on workspace rows.
pub(super) const ACTION_BTN: f32 = 20.;
/// Two action buttons (no gap) — keeps dirt gutter and hover overlay aligned.
pub(super) const ACTIONS_W: f32 = ACTION_BTN * 2.;

pub(super) fn workspace_rename_row(
    id: WorkspaceId,
    field: gpui::Entity<crate::rename::RenameView>,
    cx: &mut Context<XenonApp>,
) -> gpui::AnyElement {
    let colors = cx.theme().colors().clone();
    div()
        .id(("ws-row", id_hash(id.to_string())))
        .flex()
        .items_center()
        .h(px(ROW_H))
        .mx_1()
        .pl_2()
        .pr(px(2.))
        .rounded_sm()
        .child(
            div()
                .text_color(colors.text_muted)
                .child(icon(Icon::Folder, px(ICON_SM))),
        )
        .child(div().flex_1().min_w_0().child(field))
        .into_any_element()
}

/// Right-edge gutter for workspace rows: holds git +/- flush right.
/// `min_w` matches hover-action width so absolute actions never cover the title.
pub(super) fn workspace_dirt_gutter(
    dirt: Option<(String, String)>,
    group: &str,
    colors: &theme::ThemeColors,
) -> impl IntoElement + use<> {
    let group = group.to_string();
    div()
        .flex_none()
        .w(px(ACTIONS_W))
        .flex()
        .items_center()
        .justify_end()
        .children(dirt.map(|(plus, minus)| {
            div()
                .group_hover(group, |s| s.invisible())
                .child(crate::git_dirt::badge(
                    plus,
                    minus,
                    colors.version_control_added,
                    colors.version_control_deleted,
                ))
        }))
}

pub(super) fn path_tooltip(
    path: SharedString,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    move |_window: &mut Window, cx: &mut App| cx.new(|_| PathTooltip { text: path.clone() }).into()
}

pub(super) fn drag_chip<T: 'static>(
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

pub(super) fn id_hash(id: String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
