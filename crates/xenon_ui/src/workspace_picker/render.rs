//! Palette rendering for the Open Workspace picker.

use super::*;

impl Render for WorkspacePickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout::default();
        let empty = self.empty_message();
        let typed_folder = self.typed_folder_selected();
        let mut ranker = nucleo::Matcher::new(nucleo::Config::DEFAULT);
        let mut rows: Vec<_> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                let selectable = cand.selectable();
                let selected =
                    i == self.selected && !typed_folder && (selectable || cand.is_closed());
                let title = cand.name();
                let hits = match_hits(&title, self.hit_needle(), &mut ranker);
                let mut row = query_row(
                    ("workspace-row", i),
                    QueryRow {
                        title,
                        detail: cand.badge().map(str::to_string),
                        subtitle: self.subtitle_of(cand),
                        selected,
                        enabled: selectable || cand.is_closed(),
                        hits,
                    },
                    cx,
                );
                if selectable {
                    row = row.on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        this.moved = true;
                        this.confirm(cx);
                    }));
                } else if cand.is_closed() {
                    row = row.on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = i;
                        cx.notify();
                    }));
                }
                row.into_any_element()
            })
            .collect();
        if let Some(row) = self.ssh_row(cx) {
            rows.insert(0, row);
        }
        rows.extend(self.status_row(cx));
        let browse = xenon_design_system::action_button(
            "workspace-picker-browse-btn",
            xenon_design_system::ActionButton::quiet(xenon_design_system::shortcut_text(
                "Browse… ⌘⇧O",
            ))
            .hover_text(colors.text)
            .hover_background(gpui::transparent_black()),
            cx,
            cx.listener(|_, _, _, cx| cx.emit(WorkspacePickerEvent::Browse)),
        )
        .into_any_element();

        palette_overlay(
            PaletteOverlay {
                id: "workspace-picker-scrim",
                layout,
                colors: &colors,
                focus: self.focus.clone(),
                key_context: "WorkspacePicker",
                on_key: Self::on_key,
                on_dismiss: |_, _, _, cx| cx.emit(WorkspacePickerEvent::Dismissed),
                children: vec![
                    self.input.clone().into_any_element(),
                    scroll_results(ScrollResults {
                        list_id: "workspace-picker-results",
                        empty_message: &empty,
                        rows,
                        selected: self.selected,
                        scroll: &self.scroll,
                        colors: &colors,
                    }),
                    query_hint_action(
                        &xenon_design_system::shortcut_text(&self.hint()),
                        browse,
                        cx,
                    )
                    .into_any_element(),
                ],
            },
            cx,
        )
    }
}
