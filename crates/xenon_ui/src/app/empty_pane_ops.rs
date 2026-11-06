//! Operations for intentionally empty panes in the content tree.

use super::*;
use xenon_core::{PaneId, ParkOutcome, SplitAxis};

impl XenonApp {
    /// Add a deliberately empty sibling to the right or below the focused pane.
    pub(crate) fn park_empty_pane(
        &mut self,
        axis: SplitAxis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.follow_gpui_leaf(window, cx);
        let Some(ws) = self.active else { return };
        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        match content.park_empty(axis) {
            ParkOutcome::Unchanged => {}
            ParkOutcome::NoRoom => {
                xenon_design_system::show_toast(super::toasts::split_limit(), cx);
            }
            ParkOutcome::Created => {
                self.save_layout(ws);
                cx.notify();
            }
            ParkOutcome::Added(source) => {
                self.save_layout(ws);
                self.focus_leaf_active(source, window, cx);
                cx.notify();
            }
        }
    }

    pub(crate) fn remove_focused_empty_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.active_content().and_then(|content| content.focused) else {
            return;
        };
        self.remove_empty_pane(pane, window, cx);
    }

    pub(crate) fn remove_empty_pane(
        &mut self,
        pane: PaneId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = self.active else { return };
        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        if !content.remove_empty_leaf(pane) {
            return;
        }
        self.save_layout(ws);
        self.focus_after_teardown(Some(window), cx);
        cx.notify();
    }
}
