use gpui::Context;

use super::WorklistCaptureView;

impl WorklistCaptureView {
    pub fn visual_set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.draft = text.to_owned();
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        cx.notify();
    }
}
