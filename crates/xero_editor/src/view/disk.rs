//! External file change detection for open editor buffers.

use std::time::Duration;

use gpui::{Context, Task};

use super::{Content, EditorView};
use crate::buffer::ExternalState;

/// How often to re-check the open file against disk (agent edits, etc.).
const DISK_POLL: Duration = Duration::from_millis(400);

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
                self.recompute_highlights();
                cx.notify();
            }
            Ok(ExternalState::Conflicted | ExternalState::Deleted) => {
                // Keep in-memory edits; dirty flag already updated for Deleted.
                cx.notify();
            }
            Ok(ExternalState::Unchanged) => {}
            Err(error) => {
                log::warn!(
                    "external check failed for {}: {error}",
                    buffer.path().display()
                );
            }
        }
    }
}

/// Idle task used before the poll starts (or for non-text content).
pub(super) fn idle_disk_poll() -> Task<()> {
    Task::ready(())
}
