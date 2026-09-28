//! Hosts the shared item editor for inline capture and editing.

use super::EditorView;
use crate::item_editor::{ItemEditor, ItemEditorEvent};
use gpui::{AppContext, Context, Entity};

impl EditorView {
    pub(in crate::view) fn worklist_input_open(&self) -> bool {
        self.worklist_edit.is_some()
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

    pub(super) fn worklist_start_capture(&mut self, cx: &mut Context<Self>) {
        let editor = cx.new(ItemEditor::new);
        self.worklist_error = None;
        self.worklist_host(editor, cx);
    }
}
