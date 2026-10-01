//! Section header rows: fold chevron, name, open count, and a quiet Add.

use super::{Cursor, EditorView};
use crate::worklist_file::Target;
use crate::worklist_file::entries::parse;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use xenon_design_system::{ActionButton, TypeRole, Typography, action_button, selectable_row};

pub(super) struct HeaderState {
    pub(super) index: usize,
    pub(super) title: String,
    pub(super) open: usize,
    pub(super) items: usize,
    pub(super) selected: bool,
    pub(super) folded: bool,
    /// The name is being edited in place.
    pub(super) renaming: bool,
}

/// "2 open", "all done", or nothing for an empty section.
fn count_label(open: usize, items: usize) -> Option<String> {
    match (open, items) {
        (0, 0) => None,
        (0, _) => Some("all done".into()),
        (open, _) => Some(format!("{open} open")),
    }
}

impl EditorView {
    pub(super) fn worklist_fold(
        &mut self,
        index: usize,
        fold: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = parse(&self.text()).sections.get(index).cloned() else {
            return;
        };
        let folded = self.worklist_folded.contains(&section.title);
        if fold.unwrap_or(!folded) {
            self.worklist_folded.insert(section.title);
        } else {
            self.worklist_folded.remove(&section.title);
        }
        cx.notify();
    }

    pub(super) fn render_worklist_header(
        &mut self,
        state: HeaderState,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = state.index;
        let name = match (&self.worklist_section_edit, state.renaming) {
            (Some(edit), true) => self.section_input(edit.input.clone(), colors, cx),
            _ => {
                let title = if state.title.is_empty() {
                    "Untitled"
                } else {
                    &state.title
                };
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text)
                    .child(title.to_uppercase())
                    .into_any_element()
            }
        };
        let add = action_button(
            format!("worklist-section-add-{index}"),
            ActionButton::quiet("+ Add"),
            cx,
            cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.worklist_cursor = Cursor::Header(index);
                let parsed = parse(&this.text());
                this.worklist_start_add(Target::of(&parsed.sections, Some(index)), cx);
            }),
        )
        .min_h(px(24.))
        .opacity(if state.selected { 1. } else { 0. })
        .group_hover("worklist-header", |style| style.opacity(1.));
        let hidden = state.folded && state.items > 0;
        let shell = header_shell(format!("worklist-section-{index}"), state.selected, colors)
            .group("worklist-header")
            .on_click(cx.listener(move |this, _, _, cx| {
                this.worklist_cursor = Cursor::Header(index);
                this.worklist_fold(index, None, cx);
            }))
            .child(
                div()
                    .w(px(18.))
                    .text_color(colors.text_muted)
                    .child(if state.folded { "▸" } else { "▾" }),
            )
            .child(name)
            .children(count_label(state.open, state.items).map(|label| {
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(label)
            }))
            .child(div().flex_1())
            .child(add);
        div()
            .mt_3()
            .child(shell)
            .child(div().h(px(1.)).bg(colors.border))
            .children(
                hidden.then(|| faint_note(format!("{} items hidden", state.items), colors, cx)),
            )
            .into_any_element()
    }

    /// A header row for a section that is being named and is not in the file yet.
    pub(super) fn render_worklist_new_header(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let input = self.worklist_section_edit.as_ref()?.input.clone();
        let name = self.section_input(input, colors, cx);
        let shell = header_shell("worklist-section-new".to_string(), true, colors)
            .child(div().w(px(18.)).text_color(colors.text_muted).child("▾"))
            .child(name);
        Some(
            div()
                .mt_3()
                .child(shell)
                .child(div().h(px(1.)).bg(colors.border))
                .into_any_element(),
        )
    }

    pub(super) fn render_worklist_empty_note(
        &self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        faint_note("Empty — ↵ to add".into(), colors, cx)
    }

    fn section_input(
        &self,
        input: gpui::Entity<xenon_design_system::TextInputView>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(260.))
            .type_role(TypeRole::ControlLabel, cx)
            .border_b_1()
            .border_color(colors.border_focused)
            .child(input)
            .into_any_element()
    }
}

fn header_shell(
    id: String,
    selected: bool,
    colors: &theme::ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    selectable_row(id, selected, colors).py(px(4.)).gap_2()
}

fn faint_note(
    text: String,
    colors: &theme::ThemeColors,
    cx: &mut Context<EditorView>,
) -> AnyElement {
    div()
        .pl(px(30.))
        .py_1()
        .type_role(TypeRole::ControlLabel, cx)
        .text_color(colors.text_muted.opacity(0.7))
        .child(text)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::count_label;

    #[test]
    fn counts_open_tasks_and_says_when_everything_is_done() {
        assert_eq!(count_label(2, 5).as_deref(), Some("2 open"));
        assert_eq!(count_label(0, 3).as_deref(), Some("all done"));
        assert_eq!(count_label(0, 0), None);
    }
}
