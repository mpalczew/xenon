//! Thread-safe terminal access for the phone fast path (Xenon-only).

use std::sync::{Arc, Mutex};

use gpui::{App, Context};
use terminal::RemoteHandle;
use theme::ActiveTheme;

use super::remote::snapshot_from_content;
use super::{ScreenSnapshot, State, TerminalView};

/// `Send + Sync` handle on one terminal: write input and read screens without
/// the GPUI thread. Colors resolve against the theme last given to `set_theme`.
#[derive(Clone)]
pub struct RemoteIo {
    handle: RemoteHandle,
    theme: Arc<Mutex<Arc<theme::Theme>>>,
}

impl RemoteIo {
    pub(super) fn new(handle: RemoteHandle, cx: &App) -> Self {
        Self {
            handle,
            theme: Arc::new(Mutex::new(cx.theme().clone())),
        }
    }

    /// Typed or pasted text (newlines become Enter).
    pub fn send_text(&self, text: &str) {
        self.handle.write(super::input::pty_input_bytes(text));
    }

    /// A key-row key through the normal keystroke path (app-cursor aware).
    pub fn send_named_key(&self, keystroke: &str) -> bool {
        match gpui::Keystroke::parse(keystroke) {
            Ok(keystroke) => self.handle.try_keystroke(&keystroke),
            Err(_) => false,
        }
    }

    pub fn snapshot(&self) -> ScreenSnapshot {
        let theme = self.theme.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |theme| theme.clone(),
        );
        snapshot_from_content(&self.handle.content(), &theme)
    }

    /// Resolve cell colors against `theme` from now on.
    pub fn set_theme(&self, theme: Arc<theme::Theme>) {
        if let Ok(mut current) = self.theme.lock() {
            *current = theme;
        }
    }

    /// Install (`Some`) or remove (`None`) the hook `id`, run on the PTY
    /// thread after each output batch. Hooks must not block.
    pub fn set_wake_hook(&self, id: u64, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        match hook {
            Some(hook) => self.handle.add_wake_hook(id, hook),
            None => self.handle.remove_wake_hook(id),
        }
    }
}

impl TerminalView {
    /// Thread-safe input and snapshot access for the phone fast path
    /// (None until the PTY is ready).
    pub fn remote_io(&self, cx: &App) -> Option<RemoteIo> {
        let State::Ready(terminal) = &self.state else {
            return None;
        };
        let handle = terminal.read(cx).remote_handle()?;
        Some(RemoteIo::new(handle, cx))
    }

    /// Mac-side bookkeeping for input a phone already wrote to the PTY.
    pub fn note_remote_input(&mut self, cx: &mut Context<Self>) {
        self.note_interaction(cx);
        cx.notify();
    }
}
