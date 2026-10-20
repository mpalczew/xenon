//! Workspace-row markers: the agent line on the left edge and the git dirt
//! tick on the right edge, plus the status palette the Git section shares.

use gpui::{App, Hsla, InteractiveElement, IntoElement, ParentElement, Styled, div, hsla, px};
use theme::{ActiveTheme, ThemeColors};

use super::widgets::{ACTIONS_W, ROW_H};
use crate::app::WorkspaceDot;
use crate::git_dirt::{ChangeKind, DirtState};

/// Status colors read on dark, but the stock yellow vanishes on a light
/// canvas; there it is pulled down to a legible lightness.
fn legible(color: Hsla, cx: &App) -> Hsla {
    if cx.theme().appearance().is_light() {
        hsla(color.h, color.s, color.l.min(0.34), color.a)
    } else {
        color
    }
}

/// Name/badge color for a change kind.
pub(super) fn kind_color(kind: ChangeKind, colors: &ThemeColors, cx: &App) -> Hsla {
    match kind {
        ChangeKind::Modified => legible(colors.version_control_modified, cx),
        ChangeKind::Added => colors.version_control_added,
        ChangeKind::Untracked => colors.version_control_added,
        ChangeKind::Deleted => colors.version_control_deleted,
        ChangeKind::Renamed => colors.version_control_renamed,
        ChangeKind::Conflict => colors.version_control_conflict,
    }
}

const TICK_H: f32 = 18.;
const LINE_H: f32 = 24.;

/// Right-edge git dirt tick, centered on the row: filled when the tree
/// differs from HEAD, hollow when only the index does, absent when clean.
/// The hover actions replace it in the same gutter.
pub(super) fn dirt_gutter(
    state: DirtState,
    group: &str,
    colors: &ThemeColors,
    cx: &App,
) -> impl IntoElement + use<> {
    let group = group.to_string();
    let tint = legible(colors.version_control_modified, cx);
    let tick = match state {
        DirtState::Clean => None,
        DirtState::Dirty => Some(div().bg(tint)),
        DirtState::IndexOnly => Some(div().border_2().border_color(tint)),
    };
    div()
        .flex_none()
        .w(px(ACTIONS_W))
        .flex()
        .items_center()
        .justify_end()
        .pr(px(1.))
        .children(tick.map(|tick| {
            tick.w(px(5.))
                .h(px(TICK_H))
                .rounded_full()
                .group_hover(group, |s| s.invisible())
        }))
}

/// Short agent-status line in the left margin of a workspace row: blue
/// while working, yellow when it needs you.
pub(super) fn status_line(status: WorkspaceDot, cx: &App) -> gpui::Div {
    let color = crate::chrome::status_color(cx, matches!(status, WorkspaceDot::Attention(_)));
    div()
        .absolute()
        .left(px(-7.))
        .top(px((ROW_H - LINE_H) / 2.))
        .w(px(3.))
        .h(px(LINE_H))
        .rounded_full()
        .bg(color)
}
