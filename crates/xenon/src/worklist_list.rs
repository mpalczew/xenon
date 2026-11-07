//! The worklist list view: sections with folds, items with check boxes, and
//! inline editing, over `.xenon/worklist.md`. It is the GPUI list
//! (`xenon_editor/src/view/worklist`) built from stock widgets: one
//! `GtkListBox` of header, item and editor rows. Which rows show, the checkbox
//! flip and every edit live in `xenon_store::worklist`; each change is written
//! to the file, and the Markdown tab of the same file follows through its
//! disk watching.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use anyhow::anyhow;
use gtk::{gdk, glib};
use xenon_store::worklist::entries::{Entry, parse};
use xenon_store::worklist::view::{Cursor, toggle};
use xenon_store::worklist::{CAPTURE_KEYS, SourceEdit, Target, WorklistFile, read_source};

mod draw;
mod edit;

use crate::worklist_item::ItemEdit;

enum SectionKind {
    Rename(Target),
    /// A new section after this one's items (the end of the file when None).
    New(Option<Target>),
}

/// What a section header says about its items.
struct Counts {
    open: usize,
    items: usize,
    folded: bool,
}

/// The field that names a section.
struct SectionEdit {
    kind: SectionKind,
    widget: gtk::Box,
    entry: gtk::Entry,
}

pub(crate) struct WorklistList {
    pub(crate) root: gtk::Box,
    workspace: PathBuf,
    stack: gtk::Stack,
    list: gtk::ListBox,
    error: gtk::Label,
    first: gtk::Button,
    /// The "+ Add" buttons of the header rows; only the selected row shows its own.
    adds: RefCell<Vec<(Cursor, gtk::Button)>>,
    /// Per list row: the cursor it stands for, or None for editor and note rows.
    slots: RefCell<Vec<Option<Cursor>>>,
    cursor: Cell<Option<Cursor>>,
    folded: RefCell<BTreeSet<String>>,
    edit: RefCell<Option<Rc<ItemEdit>>>,
    section: RefCell<Option<Rc<SectionEdit>>>,
    /// The source the rows were built from.
    source: RefCell<String>,
    building: Cell<bool>,
}

fn margins(widget: &impl IsA<gtk::Widget>, vertical: i32, horizontal: i32) {
    widget.set_margin_top(vertical);
    widget.set_margin_bottom(vertical);
    widget.set_margin_start(horizontal);
    widget.set_margin_end(horizontal);
}

/// Take `widget` out of the list row that holds it, so the row can go without it.
fn detach(widget: &impl IsA<gtk::Widget>) {
    let Some(parent) = widget.parent() else {
        return;
    };
    match parent.downcast_ref::<gtk::ListBoxRow>() {
        Some(row) => row.set_child(None::<&gtk::Widget>),
        None => widget.unparent(),
    }
}

fn flat(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button
}

/// A key's action, run once the key event is over.
type Act = Box<dyn FnOnce(&Rc<WorklistList>)>;

impl WorklistList {
    /// `workspace` is the folder that holds `.xenon`; `to_markdown` runs the
    /// "Markdown" button.
    pub(crate) fn new(workspace: PathBuf, to_markdown: impl Fn() + 'static) -> Rc<Self> {
        let title = gtk::Label::new(Some("Worklist"));
        title.add_css_class("title-2");
        title.set_xalign(0.0);
        let path = gtk::Label::new(Some(".xenon/worklist.md"));
        path.add_css_class("dim-label");
        path.add_css_class("monospace");
        path.set_xalign(0.0);
        let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
        names.set_hexpand(true);
        names.append(&title);
        names.append(&path);
        let markdown = gtk::Button::with_label("Markdown");
        markdown.set_valign(gtk::Align::Center);
        markdown.connect_clicked(move |_| to_markdown());
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        margins(&header, 12, 16);
        header.append(&names);
        header.append(&markdown);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.add_css_class("error");
        margins(&error, 6, 16);
        error.set_visible(false);

        let list = gtk::ListBox::new();
        list.add_css_class("rich-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&list));
        scroller.set_vexpand(true);
        let add_item = flat("+ Add an item");
        let add_section = flat("+ Section");
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        margins(&actions, 6, 12);
        actions.append(&add_item);
        actions.append(&add_section);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&actions);
        body.append(&scroller);

        let first = gtk::Button::with_label("\u{21b5} add an item");
        first.add_css_class("pill");
        first.add_css_class("suggested-action");
        first.set_halign(gtk::Align::Center);
        let capture = gtk::Label::new(Some(&format!(
            "{} from anywhere",
            xenon_keymap::display_keys(CAPTURE_KEYS)
        )));
        capture.add_css_class("dim-label");
        let hints = gtk::Box::new(gtk::Orientation::Vertical, 12);
        hints.append(&first);
        hints.append(&capture);
        let clear = adw::StatusPage::new();
        clear.set_title("All clear.");
        clear.set_description(Some("Nothing's waiting on you here."));
        clear.set_child(Some(&hints));

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&body, Some("list"));
        stack.add_named(&clear, Some("clear"));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&error);
        root.append(&stack);

        let this = Rc::new(Self {
            root,
            workspace,
            stack,
            list,
            error,
            first,
            adds: RefCell::default(),
            slots: RefCell::default(),
            cursor: Cell::new(None),
            folded: RefCell::default(),
            edit: RefCell::new(None),
            section: RefCell::new(None),
            source: RefCell::default(),
            building: Cell::new(false),
        });
        this.wire(&add_item, &add_section);
        this
    }

    fn wire(self: &Rc<Self>, add_item: &gtk::Button, add_section: &gtk::Button) {
        let weak = Rc::downgrade(self);
        macro_rules! on {
            ($button:expr, $this:ident => $body:expr) => {{
                let weak = weak.clone();
                $button.connect_clicked(move |_| {
                    if let Some($this) = weak.upgrade() {
                        $body
                    }
                });
            }};
        }
        on!(add_item, this => this.start_add(this.selected_target()));
        on!(add_section, this => this.start_new_section());
        on!(self.first, this => this.start_add(Target::Top));

        let weak_select = weak.clone();
        self.list.connect_selected_rows_changed(move |list| {
            let Some(this) = weak_select.upgrade() else {
                return;
            };
            if this.building.get() {
                return;
            }
            let slot = list
                .selected_row()
                .and_then(|row| this.slots.borrow().get(row.index() as usize).copied())
                .flatten();
            if let Some(cursor) = slot {
                this.cursor.set(Some(cursor));
                this.show_adds();
            }
        });
        let weak_activate = weak.clone();
        self.list.connect_row_activated(move |_, row| {
            let index = row.index() as usize;
            let weak = weak_activate.clone();
            // Deferred: both actions rebuild the list inside its own activate handler.
            glib::idle_add_local_once(move || {
                let Some(this) = weak.upgrade() else { return };
                let slot = this.slots.borrow().get(index).copied().flatten();
                match slot {
                    Some(Cursor::Header(index)) => {
                        this.cursor.set(Some(Cursor::Header(index)));
                        this.fold(index, None);
                    }
                    Some(Cursor::Item(index)) => {
                        this.cursor.set(Some(Cursor::Item(index)));
                        this.start_edit(index);
                    }
                    None => {}
                }
            });
        });

        // The list keyboard: arrows walk headers and items (the list does that
        // itself); these keys act on the selected row.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if this.input_open() || state.contains(gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            if this.on_key(key, state) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.list.add_controller(keys);
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let parsed = parse(&self.source.borrow());
        let cursor = self.cursor.get();
        let item = |index: usize| (index < parsed.entries.len()).then_some(index);
        // Deferred: every action rebuilds the list that is handling this key.
        let act: Act = match (key, cursor) {
            (gdk::Key::Return | gdk::Key::KP_Enter, _) if shift => {
                Box::new(|t| t.start_new_section())
            }
            (gdk::Key::Left, Some(Cursor::Header(index))) => {
                Box::new(move |t| t.fold(index, Some(true)))
            }
            (gdk::Key::Right, Some(Cursor::Header(index))) => {
                Box::new(move |t| t.fold(index, Some(false)))
            }
            (gdk::Key::space, Some(Cursor::Header(index))) => {
                Box::new(move |t| t.fold(index, None))
            }
            (gdk::Key::Return | gdk::Key::KP_Enter, Some(Cursor::Header(index))) if ctrl => {
                Box::new(move |t| t.start_rename(index))
            }
            (gdk::Key::Return | gdk::Key::KP_Enter, Some(Cursor::Header(index))) => {
                let target = Target::of(&parsed.sections, Some(index));
                Box::new(move |t| t.start_add(target))
            }
            (gdk::Key::space, Some(Cursor::Item(index))) if item(index).is_some() => {
                Box::new(move |t| t.toggle(index))
            }
            (
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::Delete | gdk::Key::KP_Delete,
                Some(Cursor::Item(index)),
            ) if item(index).is_some() => Box::new(move |t| t.start_edit(index)),
            (gdk::Key::Return | gdk::Key::KP_Enter, _) => Box::new(|t| t.start_add(Target::Top)),
            _ => return false,
        };
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(this) = weak.upgrade() {
                act(&this);
            }
        });
        true
    }

    fn input_open(&self) -> bool {
        self.edit.borrow().is_some() || self.section.borrow().is_some()
    }

    /// Show what is on disk now, from the top; used when the view is switched to.
    pub(crate) fn show(self: &Rc<Self>) {
        self.rebuild(read_source(&self.workspace), false);
    }

    /// The file may have changed on disk. Nothing happens while an editor is
    /// open (it would lose its place); its close shows the new rows.
    pub(crate) fn refresh(self: &Rc<Self>) {
        let source = read_source(&self.workspace);
        if !self.input_open() && source != *self.source.borrow() {
            let focused = self.root.focus_child().is_some();
            self.rebuild(source, focused);
        }
    }

    /// Put the cursor where typing goes: an open editor, else the selected row.
    pub(crate) fn focus(&self) {
        let edit = self.edit.borrow().clone();
        let section = self.section.borrow().clone();
        if let Some(edit) = edit {
            edit.title.grab_focus();
        } else if let Some(section) = section {
            section.entry.grab_focus();
        } else if self.stack.visible_child_name().as_deref() == Some("clear") {
            self.first.grab_focus();
        } else {
            self.focus_cursor();
        }
    }

    fn focus_cursor(&self) {
        let at = self
            .slots
            .borrow()
            .iter()
            .position(|slot| *slot == self.cursor.get() && slot.is_some());
        if let Some(row) = at.and_then(|at| self.list.row_at_index(at as i32)) {
            row.grab_focus();
        }
    }

    fn show_adds(&self) {
        for (cursor, button) in self.adds.borrow().iter() {
            button.set_visible(self.cursor.get() == Some(*cursor));
        }
    }

    fn set_error(&self, error: Option<&str>) {
        self.error.set_text(error.unwrap_or(""));
        self.error.set_visible(error.is_some());
    }

    /// The section the selected row belongs to.
    fn selected_target(&self) -> Target {
        let parsed = parse(&self.source.borrow());
        Target::of(&parsed.sections, self.cursor_section(&parsed.entries))
    }

    fn cursor_section(&self, entries: &[Entry]) -> Option<usize> {
        match self.cursor.get()? {
            Cursor::Header(index) => Some(index),
            Cursor::Item(index) => entries.get(index).and_then(|entry| entry.section),
        }
    }

    // ---- writing ----

    /// Apply the edit `change` makes to the file as it is now. False when
    /// there was nothing to do or it failed (the error shows above the list).
    fn mutate(&self, change: impl FnOnce(&str) -> anyhow::Result<Option<SourceEdit>>) -> bool {
        let result: anyhow::Result<bool> = (|| {
            let file = WorklistFile::new(&self.workspace)?;
            let old = file.read()?;
            let source = std::str::from_utf8(old.as_deref().unwrap_or_default())?;
            let Some(edit) = change(source)? else {
                return Ok(false);
            };
            file.write(old.as_deref(), edit.apply(source).as_bytes())
                .map_err(|error| anyhow!("Worklist change was not saved: {error}"))?;
            Ok(true)
        })();
        match result {
            Ok(done) => {
                if done {
                    self.set_error(None);
                }
                done
            }
            Err(error) => {
                self.set_error(Some(&error.to_string()));
                false
            }
        }
    }

    fn toggle(self: &Rc<Self>, index: usize) {
        let done = self.mutate(|source| {
            let entries = parse(source).entries;
            Ok(entries.get(index).and_then(|entry| toggle(source, entry)))
        });
        if done {
            self.rebuild(read_source(&self.workspace), true);
        }
    }

    fn fold(self: &Rc<Self>, index: usize, fold: Option<bool>) {
        let Some(section) = parse(&self.source.borrow()).sections.get(index).cloned() else {
            return;
        };
        let folded = self.folded.borrow().contains(&section.title);
        if fold.unwrap_or(!folded) {
            self.folded.borrow_mut().insert(section.title);
        } else {
            self.folded.borrow_mut().remove(&section.title);
        }
        let source = self.source.borrow().clone();
        self.rebuild(source, true);
    }
}
