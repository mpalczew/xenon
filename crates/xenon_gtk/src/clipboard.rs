//! Clipboard commands that need more than the focused widget's own handler.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use vte::TerminalExt;

use crate::app::App;

impl App {
    /// Copy Clean: in a terminal, the selection (or what the clipboard holds)
    /// goes back to the clipboard stripped of agent TUI decoration; in an
    /// editor it is a plain copy.
    pub(crate) fn copy_clean(self: &Rc<Self>) {
        let Some(view) = self.current_term() else {
            let editor = {
                let model = self.model.borrow();
                let ws = model.active();
                let tab = ws
                    .and_then(|w| model.spaces.get(&w))
                    .and_then(|s| s.content.active_tab());
                ws.zip(tab.map(|t| t.id))
            };
            match editor.and_then(|key| self.editors.borrow().get(&key).cloned()) {
                Some(editor) => editor.view.emit_copy_clipboard(),
                None => self.toast("Nothing to copy"),
            }
            return;
        };
        if view.term.has_selection() {
            view.term.copy_clipboard_format(vte::Format::Text);
        }
        let clipboard = view.term.clipboard();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let text = clipboard.read_text_future().await.ok().flatten();
            let Some(app) = weak.upgrade() else { return };
            match text.filter(|t| !t.trim().is_empty()) {
                Some(text) => {
                    clipboard.set_text(&xenon_core::clean_agent_output(&text));
                    app.toast("Copied clean output");
                }
                None => app.toast("Nothing to copy"),
            }
        });
    }

    /// Paste into the terminal: files as quoted paths, else an image saved as
    /// a PNG and pasted as its quoted path, else the text.
    pub(crate) fn paste_terminal(self: &Rc<Self>) {
        let Some(view) = self.current_term() else {
            return;
        };
        let clipboard = view.term.clipboard();
        let formats = clipboard.formats();
        glib::spawn_future_local(async move {
            let text = if formats.contains_type(gdk::FileList::static_type()) {
                clipboard_files(&clipboard).await
            } else if formats.contains_type(gdk::Texture::static_type()) {
                clipboard_image(&clipboard).await
            } else {
                None
            };
            match text {
                Some(text) => view.term.paste_text(&text),
                None => view.term.paste_clipboard(),
            }
        });
    }
}

async fn clipboard_files(clipboard: &gdk::Clipboard) -> Option<String> {
    let value = clipboard
        .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
        .await
        .ok()?;
    let files = value.get::<gdk::FileList>().ok()?;
    let paths: Vec<_> = files.files().iter().filter_map(|f| f.path()).collect();
    (!paths.is_empty()).then(|| xenon_core::terminal_paths_text(paths))
}

async fn clipboard_image(clipboard: &gdk::Clipboard) -> Option<String> {
    let texture = clipboard.read_texture_future().await.ok().flatten()?;
    let bytes = texture.save_to_png_bytes();
    let path = xenon_core::clipboard_image_path(u64::from(glib::random_int()));
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &bytes).ok()?;
    Some(xenon_core::shell_path(path))
}
