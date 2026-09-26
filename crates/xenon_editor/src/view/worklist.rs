//! Source-preserving worklist presentation over the editor's Markdown buffer.

use gpui::prelude::FluentBuilder;
use gpui::{
    Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{
    ActionButton, CheckboxState, TypeRole, Typography, action_button, checkbox,
};

use super::{Content, EditorView};
mod capture;
mod edit;
mod edit_view;
mod source;
use gpui::Entity;
pub(super) use source::ItemEdit;
use source::{Entry, entries};
use xenon_design_system::{BulletLine, bullet_list, selectable_row};

pub(super) struct ItemForm {
    pub(super) outline: Entity<xenon_design_system::OutlineView>,
    task: bool,
}

struct RowState {
    index: usize,
    entry: Entry,
    selected: bool,
}

impl EditorView {
    pub(super) fn worklist_needs_creation(&self) -> bool {
        matches!(&self.content, Content::Text(buffer) if buffer.worklist_needs_creation())
    }

    pub(super) fn render_worklist_raw_header(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        action_button(
            "worklist-back-to-list",
            ActionButton::quiet("← Worklist    Markdown"),
            cx,
            cx.listener(|this, _, _, cx| {
                if !this.is_dirty() {
                    this.worklist_raw = false;
                    cx.notify();
                } else {
                    this.vim.ex_status =
                        Some("Save or undo Markdown edits before switching views".into());
                    cx.notify();
                }
            }),
        )
        .justify_start()
        .w_full()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(colors.border)
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_edit(&mut self, cx: &mut Context<Self>) {
        self.worklist_start_edit(0, cx);
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_capture(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.worklist_start_capture(cx);
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_markdown(&mut self, cx: &mut Context<Self>) {
        self.worklist_raw = true;
        cx.notify();
    }

    pub(super) fn render_worklist(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let body = self.render_worklist_body(&colors, cx);
        div()
            .id("worklist-view")
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .children(self.disk_alert_bar(cx))
            .children(self.worklist_error.clone().map(|error| {
                div()
                    .px_4()
                    .py_2()
                    .bg(colors.element_background)
                    .text_color(colors.version_control_deleted)
                    .child(error)
            }))
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().type_role(TypeRole::ScreenTitle, cx).child("Worklist"))
                            .child(
                                div()
                                    .type_role(TypeRole::Code, cx)
                                    .text_color(colors.text_muted)
                                    .child(".xenon/worklist.md"),
                            ),
                    )
                    .child(action_button(
                        "worklist-markdown",
                        ActionButton::secondary("Markdown"),
                        cx,
                        cx.listener(|this, _, _, cx| {
                            this.worklist_raw = true;
                            cx.notify();
                        }),
                    )),
            )
            .child(body)
            .child(self.render_worklist_footer(&colors, cx))
    }

    fn render_worklist_footer(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(colors.border)
            .type_role(TypeRole::ControlLabel, cx)
            .text_color(colors.text_muted)
            .flex()
            .justify_between()
            .child(if self.worklist_needs_creation() {
                "Empty worklist · ready to save · ↑↓ select · Space complete · Enter edit"
            } else {
                "Saved to .xenon/worklist.md · ↑↓ select · Space complete · Enter edit"
            })
            .children(self.worklist_needs_creation().then(|| {
                action_button(
                    "worklist-save-empty",
                    ActionButton::quiet("Save worklist"),
                    cx,
                    cx.listener(|this, _, _, cx| this.save(cx)),
                )
            }))
            .child(action_button(
                "worklist-undo",
                ActionButton::quiet("Undo"),
                cx,
                cx.listener(|this, _, _, cx| this.worklist_undo(false, cx)),
            ))
    }

    fn render_worklist_body(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let source = self.text();
        let rows = entries(&source);
        let selected = self.worklist_selection.min(rows.len().saturating_sub(1));
        div()
            .id("worklist-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .child(div().flex().mb_3().child(action_button(
                "worklist-add",
                ActionButton::quiet("+ Add a task or note"),
                cx,
                cx.listener(|this, _, _, cx| this.worklist_start_capture(cx)),
            )))
            .children(
                self.worklist_capture
                    .is_some()
                    .then(|| self.render_worklist_capture(cx)),
            )
            .children(rows.iter().enumerate().map(|(index, entry)| {
                if self.worklist_edit.is_some() && index == selected {
                    self.render_worklist_edit(colors, cx).into_any_element()
                } else {
                    self.render_worklist_row(
                        RowState {
                            index,
                            entry: entry.clone(),
                            selected: index == selected,
                        },
                        colors,
                        cx,
                    )
                    .into_any_element()
                }
            }))
            .when(rows.is_empty(), |view| {
                view.child(
                    div()
                        .py_8()
                        .text_color(colors.text_muted)
                        .child("Room for the next thought. Add a task or leave yourself a note."),
                )
            })
            .into_any_element()
    }

    pub(super) fn on_worklist_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let extend = event.keystroke.modifiers.shift;
        if self.worklist_input_open() {
            return false;
        }
        let count = entries(&self.text()).len();
        if event.keystroke.modifiers.platform && key == "z" {
            self.worklist_undo(extend, cx);
            return true;
        }
        match key {
            "up" => self.worklist_selection = self.worklist_selection.saturating_sub(1),
            "down" => {
                self.worklist_selection = (self.worklist_selection + 1).min(count.saturating_sub(1))
            }
            "space" if count > 0 => self.worklist_toggle(self.worklist_selection, cx),
            "enter" if count > 0 => self.worklist_start_edit(self.worklist_selection, cx),
            "delete" if count > 0 => self.worklist_start_edit(self.worklist_selection, cx),
            _ => return false,
        }
        cx.notify();
        true
    }

    fn render_worklist_row(
        &mut self,
        row: RowState,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let RowState {
            index,
            entry,
            selected,
        } = row;
        let checked = entry.checked;
        let editable = entry.editable;
        let title = entry.title.clone();
        let lines = xenon_design_system::points_from_details(&entry.details)
            .into_iter()
            .map(|point| BulletLine {
                depth: point.depth,
                text: point.text.into(),
            })
            .collect();
        selectable_row(format!("worklist-row-{index}"), selected, colors)
            .items_start()
            .mb_1()
            .child(if let Some(done) = checked {
                checkbox(
                    format!("worklist-check-{index}"),
                    CheckboxState {
                        checked: done,
                        disabled: false,
                    },
                    format!("Complete {title}"),
                    cx,
                    cx.listener(move |this, _, _, cx| {
                        this.worklist_selection = index;
                        this.worklist_toggle(index, cx);
                    }),
                )
                .into_any_element()
            } else {
                div().w(px(18.)).child("•").into_any_element()
            })
            .child(
                div()
                    .id(format!("worklist-text-{index}"))
                    .flex_1()
                    .min_w_0()
                    .text_color(if checked == Some(true) {
                        colors.text_muted
                    } else {
                        colors.text
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().type_role(TypeRole::ListPrimary, cx).child(title))
                            .child(bullet_list(lines, cx)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.worklist_selection = index;
                        if editable {
                            this.worklist_start_edit(index, cx);
                        }
                    })),
            )
            .child(action_button(
                format!("worklist-edit-{index}"),
                ActionButton::quiet("Edit").disabled(!editable),
                cx,
                cx.listener(move |this, _, _, cx| {
                    this.worklist_selection = index;
                    this.worklist_start_edit(index, cx);
                }),
            ))
    }
}
