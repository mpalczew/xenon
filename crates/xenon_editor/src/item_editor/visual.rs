use gpui::Context;

use super::ItemEditor;

impl ItemEditor {
    pub fn visual_replace(&mut self, title: &str, details: &str, cx: &mut Context<Self>) {
        self.outline
            .update(cx, |outline, cx| outline.replace(title, details, cx));
    }
}
