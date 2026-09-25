//! Focus on every opening of a reusable overlay, including reopen.

use gpui::{Context, FocusHandle, Window};

pub struct FocusOnOpen {
    handle: FocusHandle,
    pending: bool,
}

impl FocusOnOpen {
    pub fn new(handle: FocusHandle) -> Self {
        Self {
            handle,
            pending: false,
        }
    }

    pub fn open(&mut self) {
        self.pending = true;
    }

    pub fn focus_after_open<V>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        if self.pending {
            self.handle.focus(window, cx);
            self.pending = false;
        }
    }

    pub fn handle(&self) -> FocusHandle {
        self.handle.clone()
    }
}
