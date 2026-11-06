//! What the shell chrome (sidebar, tab chips, overflow menu) shows for a terminal.
//!
//! GPUI redraws a window whenever an entity read during its last draw notifies.
//! The terminal view notifies on every repaint tick, so chrome that read it made
//! each output burst, even in a hidden workspace, redraw the whole window. The
//! view sends this summary instead, only when it changes, and the app caches it.

use gpui::{App, Context};

use super::{TerminalEvent, TerminalView};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalChrome {
    pub title: String,
    pub working: bool,
    pub exited: bool,
    /// Device name while a phone owns the grid.
    pub phone_device: Option<String>,
}

impl TerminalView {
    pub fn chrome(&self, cx: &App) -> TerminalChrome {
        TerminalChrome {
            title: self.title(cx),
            working: self.working,
            exited: self.exited,
            phone_device: self.active_phone_fit().map(|fit| fit.device.to_string()),
        }
    }

    /// Emit `ChromeChanged` when the summary differs from the last one sent.
    pub(super) fn sync_chrome(&mut self, cx: &mut Context<Self>) {
        let chrome = self.chrome(cx);
        if self.chrome_sent.as_ref() == Some(&chrome) {
            return;
        }
        self.chrome_sent = Some(chrome.clone());
        cx.emit(TerminalEvent::ChromeChanged(chrome));
    }
}
