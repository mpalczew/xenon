use std::rc::Rc;

use gtk::glib;

use xenon_worklist::{Applied, Effect, ItemKey, ItemText, Op, Place, apply};

use super::list::WorklistList;
use crate::dialogs::{Prompt, ask_name};

fn check_section(text: &str) -> Result<String, String> {
    match text.trim() {
        "" => Err("Enter a name.".into()),
        name => Ok(name.to_string()),
    }
}

impl WorklistList {
    fn mutate(self: &Rc<Self>, undo: &str, op: Op) -> bool {
        match apply(&self.workspace, op) {
            Ok(Applied { now, effect }) => {
                self.rebuild(now);
                match effect {
                    Effect::Saved { undo: pair, .. } => {
                        self.set_error(None);
                        if !undo.is_empty()
                            && let Some(app) = self.app.upgrade()
                        {
                            app.offer_worklist_undo(self.workspace.clone(), undo, pair);
                        }
                        true
                    }
                    Effect::Unchanged => false,
                    Effect::Gone => {
                        self.set_error(Some("That item changed elsewhere."));
                        false
                    }
                }
            }
            Err(error) => {
                self.set_error(Some(&error.to_string()));
                false
            }
        }
    }

    fn section_at(&self, group: usize) -> Option<(xenon_worklist::SectionKey, String)> {
        let snapshot = self.snapshot.borrow();
        let group = snapshot.groups.get(group)?;
        match &group.place {
            Place::Section(key) => Some((key.clone(), group.label.clone())),
            Place::Top => None,
        }
    }

    pub(super) fn toggle(self: &Rc<Self>, item: ItemKey, done: bool) {
        if !self.mutate("", Op::SetDone { item, done }) {
            self.redraw();
        }
    }

    pub(super) fn add(self: &Rc<Self>, to: &Place, title: &str) {
        if title.trim().is_empty() {
            return;
        }
        match ItemText::task(title) {
            Ok(text) => {
                self.mutate(
                    "",
                    Op::Add {
                        to: to.clone(),
                        text,
                    },
                );
            }
            Err(error) => self.set_error(Some(&error.to_string())),
        }
    }

    pub(super) fn delete(self: &Rc<Self>, row: usize) {
        if let Some(item) = self.row_item(row) {
            self.mutate("Item deleted", Op::Remove { item: item.key });
        }
    }

    pub(super) fn move_to(self: &Rc<Self>, row: usize, group: usize) {
        let to = self
            .snapshot
            .borrow()
            .groups
            .get(group)
            .map(|g| g.place.clone());
        if let (Some(item), Some(to)) = (self.row_item(row), to) {
            self.mutate("", Op::Move { item: item.key, to });
        }
    }

    pub(super) fn delete_section(self: &Rc<Self>, group: usize) {
        if let Some((section, _)) = self.section_at(group) {
            self.mutate("Section deleted", Op::DeleteSection { section });
        }
    }

    pub(super) fn rename(self: &Rc<Self>, group: usize) {
        let Some((section, label)) = self.section_at(group) else {
            return;
        };
        let prompt = Prompt {
            title: "Rename Section",
            detail: None,
            ok: "Rename",
            initial: label,
        };
        self.ask_section(prompt, move |this, title| {
            this.mutate("", Op::RenameSection { section, title });
        });
    }

    pub(super) fn new_section(self: &Rc<Self>) {
        let prompt = Prompt {
            title: "New Section",
            detail: None,
            ok: "Add",
            initial: String::new(),
        };
        self.ask_section(prompt, |this, title| {
            this.mutate("", Op::AddSection { title });
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
            app.capture_worklist_into(self.current_place());
        }
    }

    pub(super) fn edit(self: &Rc<Self>, row: usize) {
        let Some(item) = self.row_item(row).filter(|item| item.editable) else {
            return;
        };
        if let Some(app) = self.app.upgrade() {
            app.edit_worklist_item(item);
        }
    }
}
