use std::rc::Rc;

use gtk::glib;

use super::doc::{Doc, Target};
use super::file::{Update, try_update};
use super::item::Draft;
use super::list::WorklistList;
use crate::dialogs::{Prompt, ask_name};

fn check_section(text: &str) -> Result<String, String> {
    match text.trim() {
        "" => Err("Enter a name.".into()),
        name => Ok(name.to_string()),
    }
}

impl WorklistList {
    fn mutate(
        self: &Rc<Self>,
        undo: &str,
        change: impl FnOnce(&Doc) -> anyhow::Result<Option<String>>,
    ) -> bool {
        let result = {
            let doc = self.doc.borrow();
            try_update(&self.workspace, doc.source(), || change(&doc))
        };
        match result {
            Ok(Update::Saved(pair)) => {
                self.set_error(None);
                self.rebuild(pair.after.clone());
                if !undo.is_empty()
                    && let Some(app) = self.app.upgrade()
                {
                    app.offer_worklist_undo(self.workspace.clone(), undo, pair);
                }
                true
            }
            Ok(Update::Stale(fresh)) => {
                self.rebuild(fresh);
                self.set_error(Some("The worklist changed on disk. Try that again."));
                false
            }
            Ok(Update::Noop) => false,
            Err(error) => {
                self.set_error(Some(&error.to_string()));
                false
            }
        }
    }

    pub(super) fn toggle(self: &Rc<Self>, index: usize) {
        if !self.mutate("", |doc| Ok(doc.toggle_item(index))) {
            self.redraw();
        }
    }

    pub(super) fn add(self: &Rc<Self>, target: &Target, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.mutate("", |doc| doc.add_item(target, text).map(Some));
    }

    pub(super) fn delete(self: &Rc<Self>, index: usize) {
        self.mutate("Item deleted", |doc| doc.remove_item(index).map(Some));
    }

    pub(super) fn move_to(self: &Rc<Self>, index: usize, target: usize) {
        self.mutate("", |doc| doc.move_item(index, target).map(Some));
    }

    pub(super) fn delete_section(self: &Rc<Self>, index: usize) {
        let target = self.doc.borrow().section_target(index);
        self.mutate("Section deleted", |doc| {
            doc.delete_section(&target).map(Some)
        });
    }

    pub(super) fn rename(self: &Rc<Self>, index: usize) {
        let target = self.doc.borrow().section_target(index);
        let current = target.label().to_owned();
        let prompt = Prompt {
            title: "Rename Section",
            detail: None,
            ok: "Rename",
            initial: current,
        };
        self.ask_section(prompt, move |this, name| {
            this.mutate("", |doc| doc.rename_section(&target, &name).map(Some));
        });
    }

    pub(super) fn new_section(self: &Rc<Self>) {
        let prompt = Prompt {
            title: "New Section",
            detail: None,
            ok: "Add",
            initial: String::new(),
        };
        self.ask_section(prompt, |this, name| {
            this.mutate("", |doc| doc.add_section(&name).map(Some));
        });
    }

    fn ask_section(
        self: &Rc<Self>,
        prompt: Prompt,
        done: impl FnOnce(&Rc<Self>, String) + 'static,
    ) {
        let (weak, parent) = (Rc::downgrade(self), self.root.clone());
        glib::spawn_future_local(async move {
            let name = ask_name(&parent, prompt, check_section).await;
            if let (Some(name), Some(this)) = (name, weak.upgrade()) {
                done(&this, name);
            }
        });
    }

    pub(super) fn add_via_form(self: &Rc<Self>) {
        if let Some(app) = self.app.upgrade() {
            app.capture_worklist_into(self.current_target());
        }
    }

    pub(super) fn edit(self: &Rc<Self>, index: usize) {
        let doc = self.doc.borrow();
        let Some(entry) = doc.entries.get(index).cloned() else {
            return;
        };
        if !entry.editable {
            return;
        }
        let draft = Draft::existing(doc.source(), &entry);
        drop(doc);
        if let Some(app) = self.app.upgrade() {
            app.edit_worklist_item(entry, draft);
        }
    }
}
