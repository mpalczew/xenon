//! Column-resize drag handles for the workspace sidebar, file tree, and
//! terminal/editor split. Near either edge, a drag snaps the adjacent panel
//! closed instead of letting a pane go off-screen.

use gpui::{
    AppContext, Context, Empty, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, px,
};

/// Which vertical edge is being dragged.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ResizeEdge {
    Sidebar,
    Tree,
    Terminal,
}

/// Result of a resize drag against available width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DragResult {
    /// Keep the panel open at this width.
    Width(f32),
    /// Close the pane whose right edge is being dragged.
    ClosePrimary,
    /// Close the pane to the right (editor, when the terminal grows too far).
    CloseSecondary,
}

/// Distance from an edge at which a drag snaps closed (px).
pub(crate) const SNAP_PX: f32 = 72.;
/// Minimum width kept for the main area when the sidebar is open.
const MIN_MAIN: f32 = 280.;
/// Minimum width kept for the editor body beside the file tree.
const MIN_EDITOR_BODY: f32 = 160.;
/// Minimum width kept for the editor beside the terminal.
const MIN_EDITOR: f32 = 200.;
const MIN_SIDEBAR: f32 = 140.;
const MIN_TREE: f32 = 120.;
const MIN_TERMINAL: f32 = 200.;
const MAX_SIDEBAR: f32 = 480.;
const MAX_TREE: f32 = 480.;

/// Invisible drag ghost required by GPUI's `on_drag` constructor.
struct ResizeGhost;

impl Render for ResizeGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

const HANDLE_W: f32 = 5.;

/// Which side of a `.relative()` parent the hit target sits on.
#[derive(Clone, Copy)]
pub(crate) enum HandleSide {
    Left,
    Right,
}

/// Hit target on the right edge of a pane with a 1px rule. Parent must be
/// `.relative()`.
pub(crate) fn col_resize_handle(
    id: &'static str,
    edge: ResizeEdge,
    color: Hsla,
) -> impl IntoElement {
    col_resize_handle_at(id, edge, color, HandleSide::Right)
}

/// Like [`col_resize_handle`], but on either edge (left residual handles reopen
/// a snap-closed pane).
pub(crate) fn col_resize_handle_at(
    id: &'static str,
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

/// Map a drag position to a width or a snap-close.
pub(crate) fn resolve_drag(edge: ResizeEdge, raw: f32, available: f32) -> DragResult {
    if raw < SNAP_PX {
        return DragResult::ClosePrimary;
    }
    match edge {
        ResizeEdge::Sidebar => {
            let max = (available - MIN_MAIN).clamp(MIN_SIDEBAR, MAX_SIDEBAR);
            DragResult::Width(raw.clamp(MIN_SIDEBAR, max))
        }
        ResizeEdge::Tree => {
            let max = (available - MIN_EDITOR_BODY).clamp(MIN_TREE, MAX_TREE);
            DragResult::Width(raw.clamp(MIN_TREE, max))
        }
        ResizeEdge::Terminal => {
            if raw > available - SNAP_PX {
                return DragResult::CloseSecondary;
            }
            let max = (available - MIN_EDITOR).max(MIN_TERMINAL);
            DragResult::Width(raw.clamp(MIN_TERMINAL, max))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_snaps_closed_near_left_edge() {
        assert_eq!(
            resolve_drag(ResizeEdge::Terminal, 40., 1000.),
            DragResult::ClosePrimary
        );
    }

    #[test]
    fn terminal_snaps_editor_closed_near_right_edge() {
        assert_eq!(
            resolve_drag(ResizeEdge::Terminal, 960., 1000.),
            DragResult::CloseSecondary
        );
    }

    #[test]
    fn terminal_stays_within_available() {
        let DragResult::Width(w) = resolve_drag(ResizeEdge::Terminal, 900., 1000.) else {
            panic!("expected width");
        };
        assert!(w <= 800.);
        assert!(w >= MIN_TERMINAL);
    }

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
    fn drag_back_from_snap_yields_width() {
        // Past SNAP_PX again after a close: valid width so the pane can reopen.
        assert!(matches!(
            resolve_drag(ResizeEdge::Terminal, 300., 1000.),
            DragResult::Width(_)
        ));
        assert!(matches!(
            resolve_drag(ResizeEdge::Sidebar, 180., 1200.),
            DragResult::Width(_)
        ));
        assert!(matches!(
            resolve_drag(ResizeEdge::Tree, 160., 800.),
            DragResult::Width(_)
        ));
    }
}
