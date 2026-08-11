//! Resize drag handles for the workspace sidebar and content pane splits.

use gpui::{
    AppContext, Context, Empty, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use xenon_core::{PaneId, SplitAxis};

/// Which edge is being dragged.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ResizeEdge {
    Sidebar,
    /// Vertical split between Workspaces list and Files tree in the sidebar.
    SidebarSections,
    /// Content split: axis + first child's leaf id (for ratio updates).
    Content {
        axis: SplitAxis,
        first_leaf: PaneId,
    },
}

/// Result of a sidebar resize drag against available width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DragResult {
    Width(f32),
    ClosePrimary,
}

pub(crate) const SNAP_PX: f32 = 72.;
const MIN_MAIN: f32 = 280.;
const MIN_SIDEBAR: f32 = 140.;
const MAX_SIDEBAR: f32 = 480.;

/// Workspaces section header height (sidebar).
pub(crate) const WORKSPACES_HEADER_H: f32 = 32.;
/// Files section header height (sidebar).
pub(crate) const FILES_HEADER_H: f32 = 28.;
/// Minimum Workspaces list body height when Files is open.
pub(crate) const MIN_WORKSPACES_BODY: f32 = 48.;
/// Minimum Files tree body height when both sections are open.
pub(crate) const MIN_FILES_BODY: f32 = 96.;

struct ResizeGhost;

impl Render for ResizeGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

const HANDLE_W: f32 = 5.;

#[derive(Clone, Copy)]
pub(crate) enum HandleSide {
    Left,
    Right,
}

pub(crate) fn col_resize_handle(
    id: impl Into<gpui::ElementId>,
    edge: ResizeEdge,
    color: Hsla,
) -> impl IntoElement {
    col_resize_handle_at(id, edge, color, HandleSide::Right)
}

pub(crate) fn col_resize_handle_at(
    id: impl Into<gpui::ElementId>,
    edge: ResizeEdge,
    color: Hsla,
    side: HandleSide,
) -> impl IntoElement {
    let handle = div()
        .id(id)
        .absolute()
        .top_0()
        .w(px(HANDLE_W))
        .h_full()
        .flex()
        .justify_center()
        .cursor_col_resize()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_drag(edge, |_edge, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| ResizeGhost)
        })
        .child(div().w(px(1.)).h_full().bg(color));
    match side {
        HandleSide::Right => handle.right(px(-(HANDLE_W / 2.))),
        HandleSide::Left => handle.left(px(-(HANDLE_W / 2.))),
    }
}

pub(crate) fn row_resize_handle(
    id: impl Into<gpui::ElementId>,
    edge: ResizeEdge,
    color: Hsla,
) -> impl IntoElement {
    div()
        .id(id)
        .absolute()
        .left_0()
        .bottom(px(-(HANDLE_W / 2.)))
        .h(px(HANDLE_W))
        .w_full()
        .flex()
        .items_center()
        .cursor_row_resize()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_drag(edge, |_edge, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| ResizeGhost)
        })
        .child(div().h(px(1.)).w_full().bg(color))
}

pub(crate) fn resolve_drag(edge: ResizeEdge, raw: f32, available: f32) -> DragResult {
    if raw < SNAP_PX {
        return DragResult::ClosePrimary;
    }
    match edge {
        ResizeEdge::Sidebar => {
            let max = (available - MIN_MAIN).clamp(MIN_SIDEBAR, MAX_SIDEBAR);
            DragResult::Width(raw.clamp(MIN_SIDEBAR, max))
        }
        ResizeEdge::SidebarSections | ResizeEdge::Content { .. } => DragResult::Width(raw),
    }
}

/// Clamp Workspaces list body height for the sidebar section split.
///
/// `raw` is the desired body height (px). `sidebar_h` is the full sidebar column height.
pub(crate) fn resolve_section_height(raw: f32, sidebar_h: f32) -> f32 {
    let max = (sidebar_h - WORKSPACES_HEADER_H - FILES_HEADER_H - MIN_FILES_BODY)
        .max(MIN_WORKSPACES_BODY);
    raw.clamp(MIN_WORKSPACES_BODY, max)
}

/// Content-sized Workspaces list height (no artificial cap).
pub(crate) fn workspaces_body_content_height(workspace_count: usize, row_h: f32) -> f32 {
    // rows + end drop strip + vertical padding (py_1 ≈ 4px each side)
    let content = workspace_count as f32 * row_h + 4. + 8.;
    content.max(MIN_WORKSPACES_BODY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_snaps_closed_when_narrow() {
        assert_eq!(
            resolve_drag(ResizeEdge::Sidebar, 50., 1200.),
            DragResult::ClosePrimary
        );
    }

    #[test]
    fn sidebar_does_not_exceed_main_remainder() {
        let DragResult::Width(w) = resolve_drag(ResizeEdge::Sidebar, 2000., 800.) else {
            panic!("expected width");
        };
        assert!(w <= 800. - MIN_MAIN + 0.1);
    }

    #[test]
    fn section_height_leaves_room_for_files() {
        let h = resolve_section_height(2000., 600.);
        assert!(h <= 600. - WORKSPACES_HEADER_H - FILES_HEADER_H - MIN_FILES_BODY + 0.1);
        assert!(h >= MIN_WORKSPACES_BODY);
    }

    #[test]
    fn section_height_respects_minimum() {
        assert_eq!(resolve_section_height(10., 800.), MIN_WORKSPACES_BODY);
    }

    #[test]
    fn content_height_scales_with_count() {
        let one = workspaces_body_content_height(1, 24.);
        let ten = workspaces_body_content_height(10, 24.);
        assert!(ten > one);
        assert!(ten > 220.); // larger than the old hard cap when many workspaces
    }
}
