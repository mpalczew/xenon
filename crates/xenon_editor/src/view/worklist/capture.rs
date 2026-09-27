//! One title-and-bullets form shared by inline capture and editing.

use super::{EditorView, ItemForm};
use crate::worklist_file::WorkItem;
use gpui::{AppContext, Context, IntoElement, ParentElement, div};
use xenon_design_system::{OutlineEvent, OutlineView};

impl EditorView {
    pub(in crate::view) fn worklist_input_open(&self) -> bool {
        self.worklist_capture.is_some() || self.worklist_edit.is_some()
    }

    pub(super) fn worklist_new_form(
        &mut self,
        title: &str,
        details: &str,
        checked: Option<bool>,
        cx: &mut Context<Self>,
    ) -> ItemForm {
        self.worklist_input_sub.clear();
        let outline = cx.new(|cx| OutlineView::new(title, details, checked, cx));
        outline.update(cx, |outline, cx| outline.open_title(cx));
        self.worklist_input_sub
            .push(cx.subscribe(&outline, |this, _, event, cx| match event {
                OutlineEvent::Changed if this.worklist_edit.is_some() => {
                    this.worklist_autosave_edit(cx)
                }
                OutlineEvent::Checked(checked) if this.worklist_edit.is_some() => {
                    if let Some(edit) = &mut this.worklist_edit {
                        edit.form.checked = Some(*checked);
                        if let Some(entry) = &mut edit.entry {
                            entry.checked = Some(*checked);
                        }
                    }
                    this.worklist_autosave_edit(cx);
                }
                OutlineEvent::Changed | OutlineEvent::Checked(_) => cx.notify(),
                OutlineEvent::Submit | OutlineEvent::Cancel => this.worklist_close_edit(cx),
                OutlineEvent::Delete => this.worklist_delete(cx),
            }));
        ItemForm { outline, checked }
    }

    pub(super) fn worklist_start_capture(&mut self, cx: &mut Context<Self>) {
        self.worklist_capture = None;
        let form = self.worklist_new_form("", "", Some(false), cx);
        self.worklist_edit = Some(super::ItemEdit { form, entry: None });
        self.worklist_error = None;
        cx.notify();
    }
}

impl ItemForm {
    pub(super) fn item(&self, cx: &Context<EditorView>) -> anyhow::Result<WorkItem> {
        WorkItem::new(
            &self.outline.read(cx).title(cx),
            &self.outline.read(cx).details(cx),
            self.checked.is_some(),
        )
    }

    pub(super) fn render_fields(&self) -> impl IntoElement + use<> {
        div().child(self.outline.clone())
    }
}
