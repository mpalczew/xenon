//! Remote reads and writes never block GPUI or mark unacknowledged edits clean.
use super::{Content, DiskAlert, EditorEvent, EditorView};
use crate::Buffer;
use gpui::{App, AppContext, Context, Entity};
use std::path::PathBuf;
use xenon_ssh::{RemoteFile, RemoteRead, RemoteSnapshot};

pub(super) struct RemoteDocument {
    #[cfg(feature = "visual-tests")]
    fixture: bool,
    saved_text: String,
    dirty_cache: std::cell::Cell<Option<(u64, bool)>>,
    file: RemoteFile,
    revision: String,
    busy: bool,
    loaded: bool,
    pending_save: Option<(bool, bool)>,
}

#[cfg(feature = "visual-tests")]
impl RemoteDocument {
    pub(super) fn fixture(text: &str) -> Self {
        Self {
            fixture: true,
            saved_text: text.into(),
            dirty_cache: std::cell::Cell::new(None),
            file: xenon_ssh::SshWorkspace::new("devbox".into(), "/home/alex/project".into())
                .unwrap()
                .file(std::path::Path::new("src/main.rs")),
            revision: "fixture".into(),
            busy: false,
            loaded: true,
            pending_save: None,
        }
    }
}

impl EditorView {
    pub(super) fn remote_is_dirty(&self) -> Option<bool> {
        let remote = self.remote.as_ref()?;
        let Content::Text(buffer) = &self.content else {
            return Some(false);
        };
        if self.disk_alert == DiskAlert::Deleted {
            return Some(true);
        }
        if let Some((mutation, dirty)) = remote.dirty_cache.get()
            && mutation == buffer.mutation()
        {
            return Some(dirty);
        }
        let dirty = buffer.remote_dirty(&remote.saved_text);
        remote.dirty_cache.set(Some((buffer.mutation(), dirty)));
        Some(dirty)
    }
    pub fn build_remote(
        path: PathBuf,
        file: RemoteFile,
        autofocus: bool,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let mut view = Self::from_content(
                Content::Unsupported {
                    path,
                    reason: "Connecting to SSH workspace…".into(),
                },
                autofocus,
                cx,
            );
            view.remote = Some(RemoteDocument {
                #[cfg(feature = "visual-tests")]
                fixture: false,
                saved_text: String::new(),
                dirty_cache: std::cell::Cell::new(None),
                file,
                revision: String::new(),
                busy: false,
                loaded: false,
                pending_save: None,
            });
            view.remote_refresh(false, cx);
            view.start_disk_poll(cx);
            view
        })
    }

    pub fn is_remote(&self) -> bool {
        self.remote.is_some()
    }

    pub fn remote_busy(&self) -> bool {
        self.remote.as_ref().is_some_and(|remote| remote.busy)
    }

    pub(super) fn remote_save(&mut self, force: bool, close: bool, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let Some(remote) = &mut self.remote else {
            return;
        };
        #[cfg(feature = "visual-tests")]
        if remote.fixture {
            return;
        }
        if remote.busy {
            remote.pending_save = Some((force, close));
            return;
        }
        remote.busy = true;
        let file = remote.file.clone();
        let expected = (!force).then(|| remote.revision.clone());
        let generation = buffer.generation();
        let text = buffer.text();
        let submitted = text.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { file.write(&text, expected.as_deref()) })
                .await;
            view.update(cx, |view, cx| {
                let remote = view.remote.as_mut().unwrap();
                remote.busy = false;
                match result {
                    Ok(snapshot) => {
                        remote.saved_text = snapshot.text;
                        remote.dirty_cache.set(None);
                        remote.revision = snapshot.revision;
                        if let Content::Text(buffer) = &mut view.content {
                            buffer.remote_saved(generation, &submitted);
                        }
                        view.disk_alert = DiskAlert::None;
                        cx.emit(EditorEvent::Saved {
                            path: view.path().to_path_buf(),
                        });
                        if close && !view.is_dirty() {
                            cx.emit(EditorEvent::RequestClose { force: true });
                        }
                    }
                    Err(error) => {
                        view.disk_alert = DiskAlert::Unavailable;
                        if error.to_string().contains("File changed remotely") {
                            view.disk_alert = DiskAlert::Conflict;
                        }
                        super::ex_io::save_failed(view.path(), error, cx);
                    }
                }
                if let Some((force, close)) = view.remote.as_mut().unwrap().pending_save.take() {
                    view.remote_save(force, close, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn remote_refresh(&mut self, discard: bool, cx: &mut Context<Self>) {
        let Some(remote) = &mut self.remote else {
            return;
        };
        #[cfg(feature = "visual-tests")]
        if remote.fixture {
            return;
        }
        if remote.busy {
            return;
        }
        remote.busy = true;
        let file = remote.file.clone();
        let revision = (remote.loaded && !discard).then(|| remote.revision.clone());
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { file.read(revision.as_deref()) })
                .await;
            view.update(cx, |view, cx| {
                view.remote.as_mut().unwrap().busy = false;
                match result {
                    Ok(RemoteRead::File { snapshot }) => {
                        view.apply_remote_snapshot(snapshot, discard, cx)
                    }
                    Ok(RemoteRead::Unchanged) => {
                        if matches!(view.disk_alert, DiskAlert::Unavailable | DiskAlert::Deleted) {
                            view.disk_alert = DiskAlert::None;
                        }
                    }
                    Ok(RemoteRead::Deleted) => {
                        view.disk_alert = DiskAlert::Deleted;
                        if !view.remote.as_ref().unwrap().loaded {
                            view.content = Content::Unsupported {
                                path: view.path().to_path_buf(),
                                reason: "Remote file does not exist".into(),
                            };
                        }
                    }
                    Err(error) => {
                        if !view.remote.as_ref().unwrap().loaded {
                            view.content = Content::Unsupported {
                                path: view.path().to_path_buf(),
                                reason: format!("SSH: {error}. Retrying…"),
                            };
                        } else {
                            view.disk_alert = DiskAlert::Unavailable;
                        }
                    }
                }
                if let Some((force, close)) = view.remote.as_mut().unwrap().pending_save.take() {
                    view.remote_save(force, close, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_remote_snapshot(
        &mut self,
        snapshot: RemoteSnapshot,
        discard: bool,
        cx: &mut Context<Self>,
    ) {
        let remote = self.remote.as_ref().unwrap();
        if remote.loaded && remote.revision == snapshot.revision && !discard {
            if matches!(self.disk_alert, DiskAlert::Unavailable | DiskAlert::Deleted) {
                self.disk_alert = DiskAlert::None;
            }
            return;
        }
        if !discard && self.is_dirty() {
            self.disk_alert = DiskAlert::Conflict;
            return;
        }
        let remote = self.remote.as_mut().unwrap();
        remote.revision = snapshot.revision;
        remote.saved_text = snapshot.text.clone();
        remote.dirty_cache.set(None);
        remote.loaded = true;
        match &mut self.content {
            Content::Text(buffer) => buffer.remote_reload(&snapshot.text),
            _ => {
                self.content = Content::Text(Buffer::from_remote(
                    self.path().to_path_buf(),
                    &snapshot.text,
                ))
            }
        }
        self.disk_alert = DiskAlert::None;
        self.recompute_highlights();
        self.emit_buffer_changed(cx);
    }

    pub(super) fn remote_keep(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = &mut self.remote else {
            return;
        };
        #[cfg(feature = "visual-tests")]
        if remote.fixture {
            return;
        }
        if remote.busy {
            return;
        }
        remote.busy = true;
        let file = remote.file.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { file.read(None) })
                .await;
            view.update(cx, |view, cx| {
                let remote = view.remote.as_mut().unwrap();
                remote.busy = false;
                match result {
                    Ok(result) => {
                        remote.revision = match result {
                            RemoteRead::File { snapshot } => snapshot.revision,
                            _ => String::new(),
                        };
                        view.disk_alert = DiskAlert::None;
                    }
                    Err(error) => super::ex_io::save_failed(view.path(), error, cx),
                }
                if let Some((force, close)) = view.remote.as_mut().unwrap().pending_save.take() {
                    view.remote_save(force, close, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
