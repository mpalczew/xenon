//! The worklist file's list view inside an editor tab, and the switch to its Markdown.

use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use xenon_settings::{MarkdownMode, prose_file_prefs, set_prose_file_prefs};

use super::EditorView;
use crate::worklist_list::WorklistList;

/// Is `path` a workspace's worklist?
pub(super) fn is_worklist(path: &Path) -> bool {
    path.ends_with(".xenon/worklist.md")
}

/// A worklist file shows the list or the text, on the stack that goes into
/// `root`. Other files put just the text in `root`.
pub(super) fn pages(
    path: &Path,
    root: &gtk::Box,
    scroller: &gtk::ScrolledWindow,
) -> Option<gtk::Stack> {
    if !is_worklist(path) {
        root.append(scroller);
        return None;
    }
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(scroller, Some("text"));
    root.append(&stack);
    Some(stack)
}

impl EditorView {
    /// Add the list view and its header (with the view switch) to a
    /// worklist file's tab, and the keys that switch between the two.
    pub(super) fn attach_list(self: &Rc<Self>) {
        let Some(stack) = &self.stack else {
            return;
        };
        let Some(workspace) = self.path.parent().and_then(Path::parent) else {
            return;
        };
        let weak = Rc::downgrade(self);
        let list = WorklistList::new(workspace.to_path_buf(), self.app.clone());
        stack.add_named(&list.root, Some("list"));
        if let Some(root) = self.root.downcast_ref::<gtk::Box>() {
            root.insert_child_after(&list.header, Some(&self.bar));
        }
        let showing = prose_file_prefs(&self.path).list;
        list.show_actions(showing);
        if let Some(switch) = self.switch.as_ref() {
            // The view switch moves from its own row into the worklist header.
            if let Some(row) = switch.parent() {
                row.unparent();
            }
            list.header.prepend(switch);
        }
        let _ = self.list.set(list);
        // Alt+1 and Alt+3 leave the list for that Markdown mode; Alt+4 returns to it.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !state.contains(gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let (list, mode) = match key {
                gdk::Key::_1 => (false, Some(MarkdownMode::Prose)),
                gdk::Key::_3 => (false, Some(MarkdownMode::Source)),
                gdk::Key::_4 => (true, None),
                _ => return glib::Propagation::Proceed,
            };
            glib::idle_add_local_once(move || {
                this.set_list(list);
                if let Some(mode) = mode {
                    this.set_mode(mode);
                }
            });
            glib::Propagation::Stop
        });
        self.root.add_controller(keys);
        if showing {
            stack.set_visible_child_name("list");
            if let Some(list) = self.list.get() {
                list.show();
            }
        } else {
            stack.set_visible_child_name("text");
        }
    }

    pub(super) fn list_showing(&self) -> bool {
        self.stack
            .as_ref()
            .is_some_and(|stack| stack.visible_child_name().as_deref() == Some("list"))
    }

    /// Show the worklist as the list (true) or as Markdown (false). The list
    /// reads the file, so Markdown edits must be saved first. Remembered per file.
    pub(super) fn set_list(self: &Rc<Self>, on: bool) -> bool {
        let (Some(stack), Some(list)) = (&self.stack, self.list.get()) else {
            return false;
        };
        if on && self.buffer.is_modified() {
            if let Some(app) = self.app.upgrade() {
                app.toast_failed(
                    "Save or undo first",
                    "Markdown edits need to land before the list view.",
                );
            }
            self.sync_switch();
            return true;
        }
        list.show_actions(on);
        let mut prefs = prose_file_prefs(&self.path);
        if prefs.list != on {
            prefs.list = on;
            if let Err(error) = set_prose_file_prefs(&self.path, prefs) {
                eprintln!("[xg] could not remember the worklist view: {error}");
            }
        }
        stack.set_visible_child_name(if on { "list" } else { "text" });
        if on {
            list.show();
        }
        self.sync_switch();
        self.focus();
        true
    }

    /// Put the cursor where typing goes: the list when it shows, else the text.
    pub(crate) fn focus(&self) {
        match self.list.get() {
            Some(list) if self.list_showing() => list.focus(),
            _ => {
                self.view.grab_focus();
            }
        }
    }
}
