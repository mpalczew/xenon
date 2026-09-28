//! The app's transient notice: an island that drops from the toolbar edge.
//!
//! The view owns lifetime, hover pause, the countdown ring, motion, and
//! dismissal when its action runs. Callers only say what to show.

use gpui::{Action, SharedString};

use crate::Shortcut;

mod host;
mod paint;
mod view;

pub use host::{show_toast, show_toast_in, toast_host};
pub use view::ToastView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    /// Stays until dismissed, replaced, or its action runs.
    Error,
}

/// The one action a toast offers. Its shortcut is shown on the chip and runs
/// the same action from anywhere while the toast is visible.
pub struct ToastAction {
    label: SharedString,
    shortcut: Shortcut,
    action: Box<dyn Action>,
}

impl ToastAction {
    pub fn new(label: impl Into<SharedString>, shortcut: Shortcut, action: impl Action) -> Self {
        Self {
            label: label.into(),
            shortcut,
            action: Box::new(action),
        }
    }
}

pub struct Toast {
    kind: ToastKind,
    glyph: &'static str,
    title: SharedString,
    detail: Option<SharedString>,
    action: Option<ToastAction>,
}

impl Toast {
    pub fn success(glyph: &'static str, title: impl Into<SharedString>) -> Self {
        Self::new(ToastKind::Success, glyph, title)
    }

    pub fn info(glyph: &'static str, title: impl Into<SharedString>) -> Self {
        Self::new(ToastKind::Info, glyph, title)
    }

    pub fn error(glyph: &'static str, title: impl Into<SharedString>) -> Self {
        Self::new(ToastKind::Error, glyph, title)
    }

    fn new(kind: ToastKind, glyph: &'static str, title: impl Into<SharedString>) -> Self {
        Self {
            kind,
            glyph,
            title: title.into(),
            detail: None,
            action: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn action(mut self, action: ToastAction) -> Self {
        self.action = Some(action);
        self
    }

    fn sticky(&self) -> bool {
        self.kind == ToastKind::Error
    }
}
