//! The app's toast vocabulary. One place owns the copy, glyphs, and the
//! shortcut each toast offers, so every surface speaks with one voice.

use super::*;
use xenon_design_system::{Shortcut, Toast, ToastAction};

impl XenonApp {
    pub(crate) fn show_toast(&self, toast: Toast, cx: &mut Context<Self>) {
        self.toast.update(cx, |view, cx| view.show(toast, cx));
    }

    /// ⌘Z: undo the capture the toast offers; otherwise ⌘Z belongs to the focused surface.
    pub(super) fn undo_from_toast(&mut self, cx: &mut Context<Self>) {
        if self.worklist_undo.is_some() && self.toast.read(cx).offers(&crate::UndoToast) {
            self.undo_last_worklist_capture(cx);
        } else {
            cx.propagate();
        }
    }

    /// ⌘.: hide the toast; with none showing, the key belongs to the focused surface.
    pub(super) fn dismiss_toast(&mut self, cx: &mut Context<Self>) {
        if !self.toast.update(cx, |view, cx| view.dismiss(cx)) {
            cx.propagate();
        }
    }
}

pub(crate) fn no_workspace() -> Toast {
    Toast::info("🧭", "Pick a workspace first")
        .detail("This needs a home.")
        .action(ToastAction::new(
            "Open one",
            Shortcut::new("⌘⇧O"),
            crate::AddWorkspace,
        ))
}

pub(crate) fn task_added(title: Option<&str>) -> Toast {
    let toast = Toast::success("📝", "Got it, task added");
    let toast = match title {
        Some(title) => toast.detail(title.to_string()),
        None => toast,
    };
    toast.action(ToastAction::new(
        "Undo",
        Shortcut::new("⌘Z"),
        crate::UndoToast,
    ))
}

pub(crate) fn capture_undone() -> Toast {
    Toast::info("↩️", "Poof. Capture undone.")
}

pub(crate) fn worklist_error(title: &str, error: impl std::fmt::Display) -> Toast {
    Toast::error("🙈", title.to_string())
        .detail(error.to_string())
        .action(ToastAction::new(
            "Show worklist",
            Shortcut::new("⌘⌥K"),
            crate::OpenWorklist,
        ))
}

/// A failed user action: what failed, and why.
pub(crate) fn failed(title: impl Into<SharedString>, error: impl std::fmt::Display) -> Toast {
    Toast::error("🙈", title).detail(error.to_string())
}

pub(crate) fn folder_missing(root: &Path) -> Toast {
    Toast::error("🕳️", "That folder is gone").detail(root.display().to_string())
}

pub(crate) fn no_language_server() -> Toast {
    Toast::info("🔌", "No language server here")
        .detail("Definitions and diagnostics work in Rust and TypeScript.")
}

pub(crate) fn no_definition() -> Toast {
    Toast::info("🔍", "No definition found")
}

pub(crate) fn no_diagnostics() -> Toast {
    Toast::info("✨", "No diagnostics here")
}

pub(crate) fn split_limit() -> Toast {
    Toast::info("🪆", "That’s as deep as splits go")
}

pub(crate) fn copied(what: &str) -> Toast {
    Toast::success("📋", format!("Copied {what}"))
}

pub(crate) fn trashed(name: &str) -> Toast {
    Toast::success("🗑️", format!("Moved {name} to the Trash"))
}

pub(crate) fn nothing_to_copy() -> Toast {
    Toast::info("🫙", "Nothing to copy")
}
