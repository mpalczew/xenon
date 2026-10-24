use super::{Content, EditorView};

impl EditorView {
    /// The selected text, empty when nothing is selected (or no text buffer).
    /// Feeds the `${selectedText}` task variable.
    pub fn selected_text(&self) -> String {
        match &self.content {
            Content::Text(buffer) => buffer.selected_text(),
            _ => String::new(),
        }
    }
}
