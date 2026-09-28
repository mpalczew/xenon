use gpui::Context;

use super::WorklistCaptureView;

impl WorklistCaptureView {
    pub fn visual_set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let mut lines = text.lines();
        let title = lines.next().unwrap_or("").to_owned();
        let details = lines.collect::<Vec<_>>().join("\n");
        self.editor
            .update(cx, |editor, cx| editor.visual_replace(&title, &details, cx));
        cx.notify();
    }
}
