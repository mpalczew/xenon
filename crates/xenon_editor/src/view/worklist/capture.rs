//! Hosts the shared item editor for inline capture and editing.

use super::EditorView;
use crate::item_editor::{ItemEditor, ItemEditorEvent};
use crate::worklist_file::Target;
use gpui::{AppContext, Context, Entity};

impl EditorView {
    pub(in crate::view) fn worklist_input_open(&self) -> bool {
        self.worklist_edit.is_some() || self.worklist_section_edit.is_some()
    }

    pub(super) fn worklist_host(&mut self, editor: Entity<ItemEditor>, cx: &mut Context<Self>) {
        editor.update(cx, |editor, cx| editor.open(cx));
        self.worklist_input_sub = vec![cx.subscribe(&editor, |this, _, event, cx| match event {
            ItemEditorEvent::Save(item) => this.worklist_land(item, cx),
            ItemEditorEvent::Close => this.worklist_close_edit(cx),
            ItemEditorEvent::Delete => this.worklist_delete(cx),
        })];
        self.worklist_edit = Some(editor);
        cx.notify();
    }

    /// Opens the inline editor for a new item that lands at the end of `target`.
    pub(super) fn worklist_start_add(&mut self, target: Target, cx: &mut Context<Self>) {
        if let Target::Section { title, .. } = &target {
            self.worklist_folded.remove(title);
        }
        self.worklist_add_target = target.clone();
        let editor = cx.new(ItemEditor::new);
        self.worklist_error = None;
        self.worklist_host(editor, cx);
    }
}
