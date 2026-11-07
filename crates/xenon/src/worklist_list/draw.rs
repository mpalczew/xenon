//! Drawing the rows of the worklist list view.

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use xenon_core::outline_text::points_from_details;
use xenon_store::worklist::Target;
use xenon_store::worklist::entries::{Entry, parse};
use xenon_store::worklist::view::{Cursor, count_label, inline_markup, settle, visible};

use super::{Counts, SectionEdit, SectionKind, WorklistList, detach, flat};
use crate::worklist_item::EditKind;
use crate::worklist_item::ItemEdit;
use std::collections::BTreeSet;
use xenon_store::worklist::entries::Parsed;

/// What one rebuild draws from.
#[derive(Clone, Copy)]
struct Plan<'a> {
    parsed: &'a Parsed,
    folded: &'a BTreeSet<String>,
    edit: Option<&'a ItemEdit>,
    section_edit: Option<&'a SectionEdit>,
}

impl WorklistList {
    // ---- drawing ----

    /// Draw the rows for `source`. `focus` puts the focus on the selected row.
    pub(super) fn rebuild(self: &Rc<Self>, source: String, focus: bool) {
        self.building.set(true);
        let edit = self.edit.borrow().clone();
        let section_edit = self.section.borrow().clone();
        if let Some(edit) = &edit {
            detach(&edit.form);
        }
        if let Some(section_edit) = &section_edit {
            detach(&section_edit.widget);
        }
        self.list.remove_all();
        self.adds.borrow_mut().clear();

        let parsed = parse(&source);
        let folded = self.folded.borrow().clone();
        let rows = visible(&parsed.sections, &parsed.entries, &folded);
        let cursor = self
            .cursor
            .get()
            .or_else(|| rows.first().copied())
            .map(|cursor| settle(&rows, cursor, &parsed.entries));
        self.cursor.set(cursor);

        let mut slots: Vec<Option<Cursor>> = Vec::new();
        let mut add_row = |child: &gtk::Widget, slot: Option<Cursor>| {
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(child));
            if slot.is_none() {
                row.set_selectable(false);
                row.set_activatable(false);
            }
            self.list.append(&row);
            slots.push(slot);
        };
        let plan = Plan {
            parsed: &parsed,
            folded: &folded,
            edit: edit.as_deref(),
            section_edit: section_edit.as_deref(),
        };
        let mut adds = Vec::new();
        let regions = std::iter::once(None).chain((0..parsed.sections.len()).map(Some));
        for region in regions {
            self.fill_region(region, &plan, &mut add_row, &mut adds);
        }
        *self.slots.borrow_mut() = slots;
        *self.adds.borrow_mut() = adds;
        *self.source.borrow_mut() = source;
        self.show_adds();

        let blank = edit.is_none() && section_edit.is_none() && rows.is_empty();
        let blank = blank && parsed.entries.is_empty() && parsed.sections.is_empty();
        self.stack
            .set_visible_child_name(if blank { "clear" } else { "list" });
        let at = self
            .slots
            .borrow()
            .iter()
            .position(|slot| slot.is_some() && *slot == cursor);
        if let Some(row) = at.and_then(|at| self.list.row_at_index(at as i32)) {
            self.list.select_row(Some(&row));
        }
        self.building.set(false);
        if focus {
            self.focus();
        }
    }

    /// One section (None: the items above the first): header, items, then any open editor.
    fn fill_region(
        self: &Rc<Self>,
        region: Option<usize>,
        plan: &Plan,
        add_row: &mut dyn FnMut(&gtk::Widget, Option<Cursor>),
        adds: &mut Vec<(Cursor, gtk::Button)>,
    ) {
        let Plan {
            parsed,
            folded,
            edit,
            section_edit,
        } = *plan;
        let adding_target = match edit.map(|edit| &edit.kind) {
            Some(EditKind::Add(target)) => target.resolve(&parsed.sections).ok(),
            _ => None,
        };
        let editing_item = match edit.map(|edit| &edit.kind) {
            Some(EditKind::Existing(index)) => Some(*index),
            _ => None,
        };
        let new_section_after = section_edit.and_then(|edit| match &edit.kind {
            SectionKind::New(None) => Some(parsed.sections.len().checked_sub(1)),
            SectionKind::New(Some(target)) => target.resolve(&parsed.sections).ok(),
            SectionKind::Rename(_) => None,
        });
        let renaming = section_edit.and_then(|edit| match &edit.kind {
            SectionKind::Rename(Target::Section { index, .. }) => Some(*index),
            _ => None,
        });
        let items = parsed
            .entries
            .iter()
            .filter(|e| e.section == region)
            .count();
        let mut is_folded = false;
        if let Some(index) = region {
            let section = &parsed.sections[index];
            is_folded = folded.contains(&section.title);
            let open = parsed
                .entries
                .iter()
                .filter(|e| e.section == region && e.checked == Some(false))
                .count();
            let counts = Counts {
                open,
                items,
                folded: is_folded,
            };
            let name = (renaming == Some(index))
                .then(|| section_edit.map(|edit| &edit.widget))
                .flatten();
            let (header, add) = self.header_row(index, &section.title, counts, name);
            adds.push((Cursor::Header(index), add));
            add_row(header.upcast_ref(), Some(Cursor::Header(index)));
        }
        let adding_here = adding_target == Some(region);
        if !is_folded {
            for (index, entry) in parsed
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.section == region)
            {
                match (edit, editing_item) {
                    (Some(edit), Some(editing)) if editing == index => {
                        add_row(edit.form.upcast_ref(), None);
                    }
                    _ => add_row(
                        self.item_row(index, entry).upcast_ref(),
                        Some(Cursor::Item(index)),
                    ),
                }
            }
            if items == 0 && region.is_some() && !adding_here {
                let note = gtk::Label::new(Some("Empty \u{2014} \u{21b5} to add"));
                note.add_css_class("dim-label");
                note.set_xalign(0.0);
                note.set_margin_start(30);
                add_row(note.upcast_ref(), None);
            }
        }
        if adding_here && let Some(edit) = edit {
            add_row(edit.form.upcast_ref(), None);
        }
        if new_section_after == Some(region)
            && let Some(section_edit) = section_edit
        {
            let chevron = gtk::Label::new(Some("\u{25be}"));
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            line.append(&chevron);
            line.append(&section_edit.widget);
            add_row(line.upcast_ref(), None);
        }
    }

    /// A section header: fold chevron, name, open count, and a quiet Add.
    pub(super) fn header_row(
        self: &Rc<Self>,
        index: usize,
        title: &str,
        Counts {
            open,
            items,
            folded,
        }: Counts,
        renaming: Option<&gtk::Box>,
    ) -> (gtk::Box, gtk::Button) {
        let chevron = gtk::Label::new(Some(if folded { "\u{25b8}" } else { "\u{25be}" }));
        chevron.add_css_class("dim-label");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&chevron);
        match renaming {
            Some(widget) => line.append(widget),
            None => {
                let name = gtk::Label::new(Some(&if title.is_empty() {
                    "Untitled".to_string()
                } else {
                    title.to_uppercase()
                }));
                name.add_css_class("heading");
                line.append(&name);
            }
        }
        if let Some(label) = count_label(open, items) {
            let count = gtk::Label::new(Some(&label));
            count.add_css_class("dim-label");
            line.append(&count);
        }
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        line.append(&spacer);
        let add = flat("+ Add");
        let weak = Rc::downgrade(self);
        add.connect_clicked(move |_| {
            let weak = weak.clone();
            // Deferred: the rebuild removes this button.
            glib::idle_add_local_once(move || {
                let Some(this) = weak.upgrade() else { return };
                this.cursor.set(Some(Cursor::Header(index)));
                let target = Target::of(&parse(&this.source.borrow()).sections, Some(index));
                this.start_add(target);
            });
        });
        line.append(&add);

        let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
        block.set_margin_top(6);
        block.append(&line);
        if folded && items > 0 {
            let note = gtk::Label::new(Some(&format!("{items} items hidden")));
            note.add_css_class("dim-label");
            note.add_css_class("caption");
            note.set_xalign(0.0);
            note.set_margin_start(30);
            block.append(&note);
        }
        (block, add)
    }

    /// An item: check box (or a bullet for a note), title, points and Edit.
    pub(super) fn item_row(self: &Rc<Self>, index: usize, entry: &Entry) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let title = gtk::Label::new(None);
        title.set_markup(&inline_markup(&entry.title));
        match entry.checked {
            Some(done) => {
                let check = gtk::CheckButton::new();
                check.set_active(done);
                check.set_valign(gtk::Align::Start);
                check.set_sensitive(entry.editable);
                check.update_property(&[gtk::accessible::Property::Label(&format!(
                    "Complete {}",
                    title.text()
                ))]);
                let weak = Rc::downgrade(self);
                check.connect_toggled(move |_| {
                    let weak = weak.clone();
                    glib::idle_add_local_once(move || {
                        if let Some(this) = weak.upgrade() {
                            this.cursor.set(Some(Cursor::Item(index)));
                            this.toggle(index);
                        }
                    });
                });
                row.append(&check);
                if done {
                    title.add_css_class("dim-label");
                }
            }
            None => {
                let bullet = gtk::Label::new(Some("\u{2022}"));
                bullet.set_valign(gtk::Align::Start);
                bullet.set_width_chars(2);
                row.append(&bullet);
            }
        }
        title.set_wrap(true);
        title.set_xalign(0.0);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
        text.set_hexpand(true);
        text.append(&title);
        for point in points_from_details(&entry.details) {
            let line = gtk::Label::new(None);
            line.set_markup(&format!("\u{2022}  {}", inline_markup(&point.text)));
            line.set_wrap(true);
            line.set_xalign(0.0);
            line.set_margin_start(i32::from(point.depth) * 16);
            if entry.checked == Some(true) {
                line.add_css_class("dim-label");
            }
            text.append(&line);
        }
        row.append(&text);
        let edit = flat("Edit");
        edit.set_valign(gtk::Align::Start);
        edit.set_sensitive(entry.editable);
        let weak = Rc::downgrade(self);
        edit.connect_clicked(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.cursor.set(Some(Cursor::Item(index)));
                    this.start_edit(index);
                }
            });
        });
        row.append(&edit);
        row
    }
}
