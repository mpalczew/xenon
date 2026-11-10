use std::rc::Rc;

use adw::prelude::*;
use anyhow::anyhow;
use gtk::glib;
use xenon_store::worklist::entries::parse;
use xenon_store::worklist::view::toggle;
use xenon_store::worklist::{
    Target, WorklistFile, add_item, capture_targets, delete_section, insert_section, move_item,
    remove_item, rename_section,
};

use super::WorklistList;

impl WorklistList {
    /// Apply the edit `change` makes to the file as it is now, redraw, and
    /// offer Undo with `undo` as its text when that is not empty. False when
    /// nothing changed or it failed (the error shows above the list).
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
        self.ask_name("Rename Section", &current, "Rename", move |this, name| {
            this.mutate("", |source| {
                rename_section(source, &target, &name).map(|edit| Some(edit.apply(source)))
            });
        });
    }

    pub(super) fn new_section(self: &Rc<Self>) {
        self.ask_name("New Section", "", "Add", |this, name| {
            this.mutate("", |source| {
                insert_section(source, None, &name).map(|edit| Some(edit.apply(source)))
            });
        });
    }

    fn ask_name(
        self: &Rc<Self>,
        heading: &str,
        initial: &str,
        confirm: &str,
        done: impl Fn(&Rc<Self>, String) + 'static,
    ) {
        let dialog = adw::AlertDialog::new(Some(heading), None);
        let entry = gtk::Entry::new();
        entry.set_text(initial);
        entry.set_placeholder_text(Some("Section name"));
        entry.set_activates_default(true);
        dialog.set_extra_child(Some(&entry));
        dialog.add_responses(&[("cancel", "Cancel"), ("ok", confirm)]);
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        let parent = self.root.clone();
        glib::spawn_future_local(async move {
            let focus = entry.clone();
            glib::idle_add_local_once(move || {
                focus.grab_focus();
            });
            if dialog.choose_future(Some(&parent)).await == "ok"
                && let Some(this) = weak.upgrade()
            {
                done(&this, entry.text().to_string());
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
