//! Item editing and section naming of the worklist list view.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use xenon_store::worklist::entries::parse;
use xenon_store::worklist::view::Cursor;
use xenon_store::worklist::{
    Target, delete_empty_section, insert_section, read_source, rename_section,
};

use super::{SectionEdit, SectionKind, WorklistList};
use crate::worklist_item::{EditKind, ItemEdit};
use gtk::{gdk, glib};

impl WorklistList {
    // ---- item editor ----

    pub(super) fn start_edit(self: &Rc<Self>, index: usize) {
        let source = self.source.borrow().clone();
        let Some(entry) = parse(&source).entries.get(index).cloned() else {
            return;
        };
        if !entry.editable {
            return;
        }
        self.cursor.set(Some(Cursor::Item(index)));
        self.open_edit(ItemEdit::existing(
            self.workspace.clone(),
            (index, &entry),
            &source,
            self.close_callback(),
        ));
    }

    /// A new item at the end of `target`.
    pub(super) fn start_add(self: &Rc<Self>, target: Target) {
        if let Target::Section { title, .. } = &target {
            self.folded.borrow_mut().remove(title);
        }
        self.open_edit(ItemEdit::add(
            self.workspace.clone(),
            target,
            self.close_callback(),
        ));
    }

    pub(super) fn close_callback(self: &Rc<Self>) -> impl Fn() + 'static {
        let weak = Rc::downgrade(self);
        move || {
            if let Some(this) = weak.upgrade() {
                this.close_edit();
            }
        }
    }

    pub(super) fn open_edit(self: &Rc<Self>, edit: Rc<ItemEdit>) {
        self.set_error(None);
        let title = edit.title.clone();
        *self.edit.borrow_mut() = Some(edit);
        let source = self.source.borrow().clone();
        self.rebuild(source, false);
        title.grab_focus();
    }

    pub(super) fn close_edit(self: &Rc<Self>) {
        let Some(edit) = self.edit.borrow_mut().take() else {
            return;
        };
        let source = read_source(&self.workspace);
        // A new item lands at the end of its section.
        if let EditKind::Add(target) = &edit.kind {
            let parsed = parse(&source);
            if let Ok(region) = target.resolve(&parsed.sections)
                && let Some(last) = parsed.entries.iter().rposition(|e| e.section == region)
            {
                self.cursor.set(Some(Cursor::Item(last)));
            }
        }
        self.rebuild(source, true);
    }

    // ---- section naming ----

    pub(super) fn start_rename(self: &Rc<Self>, index: usize) {
        let parsed = parse(&self.source.borrow());
        let Some(section) = parsed.sections.get(index) else {
            return;
        };
        let target = Target::of(&parsed.sections, Some(index));
        let title = section.title.clone();
        self.begin_section(SectionKind::Rename(target), &title);
    }

    pub(super) fn start_new_section(self: &Rc<Self>) {
        let parsed = parse(&self.source.borrow());
        let section = self.cursor_section(&parsed.entries);
        let has_selection = !parsed.entries.is_empty() || !parsed.sections.is_empty();
        let after = has_selection.then(|| Target::of(&parsed.sections, section));
        self.begin_section(SectionKind::New(after), "");
    }

    pub(super) fn begin_section(self: &Rc<Self>, kind: SectionKind, initial: &str) {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("Section name"));
        entry.set_text(initial);
        entry.set_hexpand(true);
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.append(&entry);
        let weak = Rc::downgrade(self);
        entry.connect_activate({
            let weak = weak.clone();
            move |entry| {
                if let Some(this) = weak.upgrade() {
                    this.finish_section(Some(entry.text().to_string()));
                }
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed({
            let weak = weak.clone();
            move |_, key, _, _| {
                if key != gdk::Key::Escape {
                    return glib::Propagation::Proceed;
                }
                if let Some(this) = weak.upgrade() {
                    this.finish_section(None);
                }
                glib::Propagation::Stop
            }
        });
        entry.add_controller(keys);
        // Naming ends when the field loses focus.
        let blur = gtk::EventControllerFocus::new();
        blur.connect_leave({
            let entry = entry.clone();
            move |_| {
                if let Some(this) = weak.upgrade()
                    && !this.building.get()
                {
                    this.finish_section(Some(entry.text().to_string()));
                }
            }
        });
        entry.add_controller(blur);
        self.set_error(None);
        *self.section.borrow_mut() = Some(Rc::new(SectionEdit {
            kind,
            widget,
            entry: entry.clone(),
        }));
        let source = self.source.borrow().clone();
        self.rebuild(source, false);
        entry.grab_focus();
    }

    /// Closes the field; `text` is None when the user cancelled.
    pub(super) fn finish_section(self: &Rc<Self>, text: Option<String>) {
        let Some(edit) = self.section.borrow_mut().take() else {
            return;
        };
        if let Some(text) = text {
            match &edit.kind {
                SectionKind::Rename(target) => self.rename(target, &text),
                SectionKind::New(after) => self.create_section(after.as_ref(), &text),
            }
        }
        self.rebuild(read_source(&self.workspace), true);
    }

    pub(super) fn rename(&self, target: &Target, text: &str) {
        let title = text.trim();
        if title == target.label() {
            return;
        }
        let saved = self.mutate(|source| {
            if title.is_empty() {
                delete_empty_section(source, target).map(Some)
            } else {
                rename_section(source, target, title).map(Some)
            }
        });
        if saved && self.folded.borrow_mut().remove(target.label()) && !title.is_empty() {
            self.folded.borrow_mut().insert(title.to_owned());
        }
    }

    pub(super) fn create_section(&self, after: Option<&Target>, text: &str) {
        let title = text.trim();
        if title.is_empty() {
            return;
        }
        let at = Cell::new(0);
        let saved = self.mutate(|source| {
            let edit = insert_section(source, after, title)?;
            at.set(edit.range.start);
            Ok(Some(edit))
        });
        if saved {
            let sections = parse(&read_source(&self.workspace)).sections;
            let created = sections
                .iter()
                .position(|s| s.start >= at.get() && s.title == title);
            if let Some(index) = created {
                self.cursor.set(Some(Cursor::Header(index)));
            }
        }
    }
}
