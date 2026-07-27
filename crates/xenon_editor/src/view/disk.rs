//! External file change detection and dirty/disk conflict banner.

use std::time::Duration;

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Task, div,
};
use theme::ActiveTheme;

use super::{Content, EditorView};
use crate::buffer::ExternalState;

/// How often to re-check the open file against disk (agent edits, etc.).
const DISK_POLL: Duration = Duration::from_millis(400);

/// External file status when the buffer cannot silently reload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiskAlert {
    #[default]
    None,
    /// File changed on disk while the buffer has unsaved edits.
    Conflict,
    /// File was removed from disk.
    Deleted,
}

impl EditorView {
    pub(super) fn start_disk_poll(&mut self, cx: &mut Context<Self>) {
        self._disk_poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DISK_POLL).await;
                if this
                    .update(cx, |view, cx| {
                        view.sync_from_disk(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    /// Reload from disk when the file changed and this buffer is clean.
    /// Dirty buffers are left alone (conflict); deleted files mark dirty.
    pub fn sync_from_disk(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        match buffer.check_external() {
            Ok(ExternalState::Reloaded) => {
                self.disk_alert = DiskAlert::None;
                self.recompute_highlights();
                self.emit_buffer_changed(cx);
                cx.notify();
            }
            Ok(ExternalState::Conflicted) => {
                if self.disk_alert != DiskAlert::Conflict {
                    self.disk_alert = DiskAlert::Conflict;
                    cx.notify();
                }
            }
            Ok(ExternalState::Deleted) => {
                if self.disk_alert != DiskAlert::Deleted {
                    self.disk_alert = DiskAlert::Deleted;
                    cx.notify();
                }
            }
            Ok(ExternalState::Unchanged) => {
                if self.disk_alert != DiskAlert::None {
                    self.disk_alert = DiskAlert::None;
                    cx.notify();
                }
            }
            Err(error) => {
                log::warn!(
                    "external check failed for {}: {error}",
                    buffer.path().display()
                );
            }
        }
    }

    /// Drop local edits and reload from disk (user chose disk in a conflict).
    pub fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if let Err(error) = buffer.reload() {
            log::warn!("reload failed for {}: {error}", buffer.path().display());
            return;
        }
        self.disk_alert = DiskAlert::None;
        self.recompute_highlights();
        self.emit_buffer_changed(cx);
        cx.notify();
    }

    /// Keep local edits and adopt current disk mtime (user chose buffer).
    pub fn keep_local_edits(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.adopt_disk_mtime();
        self.disk_alert = DiskAlert::None;
        cx.notify();
    }

    pub(super) fn disk_alert_bar(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let (msg, show_actions) = match self.disk_alert {
            DiskAlert::None => return None,
            DiskAlert::Conflict => (
                "File changed on disk. Keep your edits, or reload from disk?",
                true,
            ),
            DiskAlert::Deleted => (
                "File was deleted on disk. Your buffer is still open.",
                false,
            ),
        };
        let colors = cx.theme().colors().clone();
        let status = cx.theme().status().clone();
        let mut bar = div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .bg(status.warning_background)
            .text_color(status.warning)
            .border_b_1()
            .border_color(status.warning_border)
            .child(div().flex_1().child(msg));
        if show_actions {
            bar = bar
                .child(
                    div()
                        .id("disk-keep")
                        .px_2()
                        .py_px()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(colors.element_hover))
                        .child("Keep mine")
                        .on_click(cx.listener(|this, _, _, cx| this.keep_local_edits(cx))),
                )
                .child(
                    div()
                        .id("disk-reload")
                        .px_2()
                        .py_px()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(colors.element_hover))
                        .child("Load disk")
                        .on_click(cx.listener(|this, _, _, cx| this.reload_from_disk(cx))),
                );
        }
        Some(bar)
    }
}

/// Idle task used before the poll starts (or for non-text content).
pub(super) fn idle_disk_poll() -> Task<()> {
    Task::ready(())
}
