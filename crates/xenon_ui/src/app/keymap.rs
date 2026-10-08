//! Live keymap: watch `~/.xenon/keymap.json`, rebuild on save, say what went
//! wrong, and open the file for editing.

use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use super::*;
use crate::keymap;

const DEBOUNCE: Duration = Duration::from_millis(150);

impl XenonApp {
    /// Report a startup file error, then follow the file for the session.
    pub(super) fn start_keymap_watch(&mut self, cx: &mut Context<Self>) {
        if let Some(error) = cx.try_global::<keymap::StartupError>() {
            let toast = super::toasts::keymap_error(&error.0);
            self.show_toast(toast, cx);
        }
        let (tx, rx) = async_channel::unbounded();
        let Some(watcher) = spawn_watcher(tx) else {
            return;
        };
        self.services.keymap_watcher = Some(watcher);
        self.services.keymap_task = Some(cx.spawn(async move |view, cx| {
            while rx.recv().await.is_ok() {
                cx.background_executor().timer(DEBOUNCE).await;
                while rx.try_recv().is_ok() {}
                if view.update(cx, |app, cx| app.reload_keymap(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn reload_keymap(&mut self, cx: &mut Context<Self>) {
        match keymap::reload(cx) {
            Ok(true) => self.show_toast(super::toasts::keymap_reloaded(), cx),
            Ok(false) => {}
            Err(message) => self.show_toast(super::toasts::keymap_error(&message), cx),
        }
    }

    pub(crate) fn open_keymap(&mut self, cx: &mut Context<Self>) {
        match keymap::ensure_file() {
            Ok(path) => self.open_editor(path, true, cx),
            Err(error) => self.show_toast(
                super::toasts::failed("Couldn’t create the keymap", error),
                cx,
            ),
        }
    }
}

/// Watch the data directory (the file may not exist yet) for keymap changes.
fn spawn_watcher(tx: async_channel::Sender<()>) -> Option<notify::RecommendedWatcher> {
    let path = keymap::keymap_path();
    let dir = path.parent()?.to_path_buf();
    let name = path.file_name()?.to_owned();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let touched = res.is_ok_and(|event| {
            event
                .paths
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == name))
        });
        if touched {
            let _ = tx.try_send(());
        }
    })
    .inspect_err(|error| log::warn!("keymap watcher failed to start: {error}"))
    .ok()?;
    if let Err(error) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        log::warn!("keymap: cannot watch {}: {error}", dir.display());
        return None;
    }
    Some(watcher)
}
