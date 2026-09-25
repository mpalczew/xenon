use super::*;

impl XenonApp {
    pub(super) fn render_leaf(
        &self,
        leaf: &LiveLeaf,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let pane_id = leaf.id;
        let has_active_tab = leaf.active_tab().is_some();
        let focused = leaf
            .active_tab()
            .is_some_and(|tab| super::keyboard::tab_has_gpui_focus(tab, window, cx));
        let tabs = self.render_mixed_tabs(leaf, focused, cx);
        let body_content = match leaf.active_tab() {
            Some(LiveTab::Terminal { view, .. }) => div()
                .size_full()
                .min_h_0()
                .min_w_0()
                .child(view.clone())
                .into_any_element(),
            Some(LiveTab::Editor { view, .. }) => div()
                .size_full()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .child(view.clone())
                .into_any_element(),
            None => self
                .render_empty_state(colors.clone(), Some(pane_id), cx)
                .into_any_element(),
        };

        let ws = self.active;
        let dragging = cx.has_active_drag();
        let drop_line = colors.drop_target_border;

        // While a tab is dragged, overlay hit-targets so terminal/editor content
        // does not swallow the drop. Center = move; edges = split.
        let drop_overlay = dragging.then(|| self.tab_drop_overlay(pane_id, ws, drop_line, cx));
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(body_content)
            .children(drop_overlay);
        div()
            .id(("leaf", leaf.id.0))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .min_w_0()
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    // Tab strip is not track_focus; without this the root
                    // XenonApp handle steals GPUI and the pane ring goes away.
                    window.prevent_default();
                    if has_active_tab {
                        this.focus_leaf_active(pane_id, window, cx);
                    } else {
                        this.adopt_focused_pane(pane_id, cx);
                    }
                }),
            )
            .child(tabs)
            .child(body)
            .into_any_element()
    }
}
