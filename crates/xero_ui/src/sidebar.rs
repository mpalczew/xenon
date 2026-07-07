//! The collapsible left column: a workspace selector. Rendered as a method on
//! `XeroApp` so click handlers can use `cx.listener`.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use theme::ActiveTheme;
use xero_core::WorkspaceId;

use crate::AddWorkspace;
use crate::app::XeroApp;

impl XeroApp {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let active = self.active_workspace();
        // Collect owned row data first so the `self.registry()` borrow is
        // released before we reborrow `cx` per row.
        let entries: Vec<(WorkspaceId, String, bool)> = self
            .registry()
            .workspaces
            .iter()
            .map(|w| (w.id, w.name.clone(), active == Some(w.id)))
            .collect();
        let mut rows = Vec::with_capacity(entries.len());
        for (id, name, is_active) in entries {
            rows.push(self.workspace_row(id, &name, is_active, cx));
        }

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.sidebar_header(cx))
            .children(rows)
    }

    fn sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(div().text_sm().child("WORKSPACES"))
            .child(
                div()
                    .id("add-workspace")
                    .px_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover))
                    .child("+")
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(AddWorkspace), cx);
                    })),
            )
    }

    fn workspace_row(
        &self,
        id: WorkspaceId,
        name: &str,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let background = if is_active {
            colors.element_selected
        } else {
            colors.panel_background
        };
        div()
            .id(("workspace", id_hash(id)))
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .text_sm()
            .bg(background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .child(name.to_string())
            .on_click(cx.listener(move |this, _, _, cx| this.activate_workspace(id, cx)))
    }
}

/// A stable element id for a workspace row.
fn id_hash(id: WorkspaceId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.to_string().hash(&mut hasher);
    hasher.finish()
}
