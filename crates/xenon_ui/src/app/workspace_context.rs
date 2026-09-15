use super::*;

impl XenonApp {
    pub(super) fn render_workspace_context(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let Some(id) = self.active_workspace() else {
            return div().h(px(0.)).into_any_element();
        };
        let Some(workspace) = self.registry().workspaces.iter().find(|w| w.id == id) else {
            return div().h(px(0.)).into_any_element();
        };
        let (working, waiting) = self.workspace_status_counts(id, cx);
        let path = workspace.root.display().to_string();
        let path = std::env::var_os("HOME")
            .and_then(|home| {
                path.strip_prefix(home.to_string_lossy().as_ref())
                    .map(|rest| {
                        if rest.is_empty() {
                            "~".to_string()
                        } else {
                            format!("~{rest}")
                        }
                    })
            })
            .unwrap_or(path);
        let status = if working > 0 {
            Some((
                "✦",
                format!("{working} agent working"),
                cx.theme().status().info,
            ))
        } else if waiting > 0 {
            Some((
                "●",
                format!("{waiting} needs you"),
                cx.theme().status().warning,
            ))
        } else {
            None
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(48.))
            .px_8()
            .border_b_1()
            .border_color(colors.border)
            .text_sm()
            .child(
                div()
                    .text_color(colors.text_accent)
                    .child(workspace.name.clone()),
            )
            .child(div().text_color(colors.text_muted).child(path))
            .children(status.map(|(glyph, label, color)| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_color(color)
                    .child(glyph)
                    .child(label)
            }))
            .into_any_element()
    }
}
