//! Clipboard commands that need more than the focused widget's own handler.

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
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
}
