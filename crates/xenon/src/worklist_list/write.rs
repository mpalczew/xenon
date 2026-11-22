use std::rc::Rc;

use anyhow::anyhow;
use gtk::glib;
use xenon_store::worklist::entries::parse;
use xenon_store::worklist::view::toggle;
use xenon_store::worklist::{
    Target, WorklistFile, add_item, capture_targets, delete_section, insert_section, move_item,
    remove_item, rename_section,
};

use super::WorklistList;
use crate::dialogs::{Prompt, ask_name};

fn check_section(text: &str) -> Result<String, String> {
    match text.trim() {
        "" => Err("Enter a name.".into()),
        name => Ok(name.to_string()),
    }
}

impl WorklistList {
    pub(super) fn mutate(
        self: &Rc<Self>,
        undo: &str,
        change: impl FnOnce(&str) -> anyhow::Result<Option<String>>,
    ) -> bool {
        let result: anyhow::Result<Option<(String, String)>> = (|| {
            let file = WorklistFile::new(&self.workspace)?;
            let old = file.read()?;
            let before = String::from_utf8(old.clone().unwrap_or_default())?;
            if before != *self.source.borrow() {
                self.rebuild(before, None);
                return Err(anyhow!("The worklist changed on disk. Try that again."));
            }
            let Some(after) = change(&before)? else {
                return Ok(None);
            };
            file.write(old.as_deref(), after.as_bytes())
                .map_err(|error| anyhow!("Worklist change was not saved: {error}"))?;
            Ok(Some((before, after)))
        })();
        match result {
            Ok(Some((before, after))) => {
                self.set_error(None);
                self.rebuild(after.clone(), None);
                if !undo.is_empty()
                    && let Some(app) = self.app.upgrade()
                {
                    app.offer_worklist_undo(self.workspace.clone(), undo, before, after);
                }
                true
            }
            Ok(None) => false,
            Err(error) => {
                self.set_error(Some(&error.to_string()));
                false
            }
        }
    }

    pub(super) fn toggle(self: &Rc<Self>, index: usize) {
        let done = self.mutate("", |source| {
            let entries = parse(source).entries;
            Ok(entries
                .get(index)
                .and_then(|entry| toggle(source, entry))
                .map(|edit| edit.apply(source)))
        });
        if !done {
            self.redraw();
        }
    }

    pub(super) fn add(self: &Rc<Self>, target: &Target, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.mutate("", |source| add_item(source, target, text).map(Some));
    }

    pub(super) fn delete(self: &Rc<Self>, index: usize) {
        self.mutate("Item deleted", |source| {
            let entry = parse(source).entries.get(index).cloned();
            let entry = entry.ok_or_else(|| anyhow!("Worklist item is gone"))?;
            remove_item(source, entry.range).map(Some)
        });
    }

    pub(super) fn move_to(self: &Rc<Self>, index: usize, target: usize) {
        self.mutate("", |source| {
            let entry = parse(source).entries.get(index).cloned();
            let entry = entry.ok_or_else(|| anyhow!("Worklist item is gone"))?;
            let target = capture_targets(source)
                .get(target)
                .cloned()
                .ok_or_else(|| anyhow!("Section is gone"))?;
            move_item(source, entry.range, &target).map(Some)
        });
    }

    fn section_target(&self, index: usize) -> Target {
        Target::of(&parse(&self.source.borrow()).sections, Some(index))
    }

    pub(super) fn delete_section(self: &Rc<Self>, index: usize) {
        let target = self.section_target(index);
        self.mutate("Section deleted", |source| {
            delete_section(source, &target).map(|edit| Some(edit.apply(source)))
        });
    }

    pub(super) fn rename(self: &Rc<Self>, index: usize) {
        let target = self.section_target(index);
        let current = target.label().to_owned();
        let prompt = Prompt {
            title: "Rename Section",
            detail: None,
            ok: "Rename",
            initial: current.clone(),
        };
        self.ask_section(prompt, move |this, name| {
            this.mutate("", |source| {
                rename_section(source, &target, &name).map(|edit| Some(edit.apply(source)))
            });
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
            this.mutate("", |source| {
                insert_section(source, None, &name).map(|edit| Some(edit.apply(source)))
            });
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
        let source = self.source.borrow().clone();
        let entry = parse(&source).entries.get(index).cloned();
        if let (Some(entry), Some(app)) = (entry, self.app.upgrade())
            && entry.editable
        {
            app.edit_worklist_item(entry, source);
        }
    }
}
