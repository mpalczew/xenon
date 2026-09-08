//! Operations for intentionally empty panes in the content tree.

use super::*;
use xenon_core::{MAX_NEST_DEPTH, PaneId, SplitAxis};

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
        if content.root.is_none() {
            let first = content.next_pane_id();
            let second = PaneId(first.0.saturating_add(1));
            content.root = Some(LiveNode::Split {
                axis,
                ratio: 0.5,
                first: Box::new(LiveNode::Leaf(LiveLeaf {
                    id: first,
                    tabs: Vec::new(),
                    active: 0,
                    parked: false,
                })),
                second: Box::new(LiveNode::Leaf(LiveLeaf {
                    id: second,
                    tabs: Vec::new(),
                    active: 0,
                    parked: true,
                })),
            });
            content.focused = Some(first);
            self.save_layout(ws);
            cx.notify();
            return;
        }
        let Some(source) = content.focused else {
            return;
        };
        let Some((depth, mut source_leaf)) = content
            .root
            .as_ref()
            .and_then(|root| Some((root.nest_depth(source)?, root.find_leaf(source)?.clone())))
        else {
            return;
        };
        if depth >= MAX_NEST_DEPTH {
            return;
        }
        source_leaf.parked = false;
        let new_pane = content.next_pane_id();
        let replacement = LiveNode::Split {
            axis,
            ratio: 0.5,
            first: Box::new(LiveNode::Leaf(source_leaf)),
            second: Box::new(LiveNode::Leaf(LiveLeaf {
                id: new_pane,
                tabs: Vec::new(),
                active: 0,
                parked: true,
            })),
        };
        if content
            .root
            .as_mut()
            .is_some_and(|root| root.replace_leaf(source, replacement))
        {
            content.focused = Some(source);
            self.save_layout(ws);
            self.focus_leaf_active(source, window, cx);
            cx.notify();
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
        if !content
            .root
            .as_ref()
            .and_then(|root| root.find_leaf(pane))
            .is_some_and(|leaf| leaf.tabs.is_empty() && content.leaf_ids().len() > 1)
        {
            return;
        }
        content.unsplit_empty(pane);
        self.save_layout(ws);
        self.focus_after_teardown(Some(window), cx);
        cx.notify();
    }
}
