//! Tab drag-and-drop targets on content leaves.

use super::*;
use gpui::SharedString;
use xenon_core::{DropEdge, PaneId, WorkspaceId};

impl XenonApp {
    /// Full-pane drop chrome: edge bands split, center moves the tab.
    pub(crate) fn tab_drop_overlay(
        &self,
        pane_id: PaneId,
        ws: Option<WorkspaceId>,
        drop_line: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        const EDGE: f32 = 40.;
        div()
            .id(("leaf-drop", pane_id.0))
            .absolute()
            .inset_0()
            .occlude()
            .child(Self::tab_drop_band(
                format!("drop-c-{}", pane_id.0),
                pane_id,
                ws,
                None,
                drop_line,
                div()
                    .absolute()
                    .top(px(EDGE))
                    .bottom(px(EDGE))
                    .left(px(EDGE))
                    .right(px(EDGE)),
                cx,
            ))
            .child(Self::tab_drop_band(
                format!("drop-l-{}", pane_id.0),
                pane_id,
                ws,
                Some(DropEdge::Left),
                drop_line,
                div().absolute().top_0().bottom_0().left_0().w(px(EDGE)),
                cx,
            ))
            .child(Self::tab_drop_band(
                format!("drop-r-{}", pane_id.0),
                pane_id,
                ws,
                Some(DropEdge::Right),
                drop_line,
                div().absolute().top_0().bottom_0().right_0().w(px(EDGE)),
                cx,
            ))
            .child(Self::tab_drop_band(
                format!("drop-t-{}", pane_id.0),
                pane_id,
                ws,
                Some(DropEdge::Top),
                drop_line,
                div()
                    .absolute()
                    .top_0()
                    .left(px(EDGE))
                    .right(px(EDGE))
                    .h(px(EDGE)),
                cx,
            ))
            .child(Self::tab_drop_band(
                format!("drop-b-{}", pane_id.0),
                pane_id,
                ws,
                Some(DropEdge::Bottom),
                drop_line,
                div()
                    .absolute()
                    .bottom_0()
                    .left(px(EDGE))
                    .right(px(EDGE))
                    .h(px(EDGE)),
                cx,
            ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tab_drop_band(
        id: String,
        pane_id: PaneId,
        ws: Option<WorkspaceId>,
        edge: Option<DropEdge>,
        drop_line: gpui::Hsla,
        base: gpui::Div,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        base.id(SharedString::from(id))
            .can_drop(move |drag, _, _| {
                drag.downcast_ref::<DragTab>()
                    .is_some_and(|d| Some(d.workspace) == ws)
            })
            .drag_over::<DragTab>(move |style, _, _, _| {
                style
                    .bg(drop_line.opacity(0.35))
                    .border_2()
                    .border_color(drop_line)
            })
            .on_drop(cx.listener(move |this, drag: &DragTab, window, cx| {
                if Some(drag.workspace) != ws {
                    return;
                }
                match edge {
                    Some(edge) => this.drop_tab_on_edge(drag.tab, pane_id, edge, window, cx),
                    None => this.move_tab_to_pane(drag.tab, pane_id, window, cx),
                }
            }))
    }
}
