//! The inline item editor of the worklist list view: check box, title and
//! points. It saves as you type through `xenon_store::worklist`, as the GPUI
//! item editor did; the capture dialog (`worklist.rs`) works the same way.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_store::worklist::entries::Entry;
use xenon_store::worklist::{
    Edit, ItemDraft, TITLE_LIMIT, Target, WorkItem, title_length, write_draft,
};

/// Characters left at which the countdown appears.
const NEAR_LIMIT: isize = 12;

/// What the editor works on.
pub(super) enum EditKind {
    /// The item at this index of the parsed entries.
    Existing(usize),
    /// A new item at the end of this section.
    Add(Target),
}

pub(super) struct ItemEdit {
    pub(super) kind: EditKind,
    pub(super) form: gtk::Box,
    pub(super) title: gtk::Entry,
    details: gtk::TextView,
    check: gtk::CheckButton,
    note: gtk::Label,
    count: gtk::Label,
    hint: gtk::Box,
    error: gtk::Label,
    root: PathBuf,
    /// A task has a check box; a note does not.
    task: bool,
    target: Target,
    draft: RefCell<ItemDraft>,
    /// The item and done state as last written.
    landed: RefCell<Option<(WorkItem, bool)>>,
    close: Box<dyn Fn()>,
}

impl ItemEdit {
    /// A new task at the end of `target`.
    pub(super) fn add(root: PathBuf, target: Target, close: impl Fn() + 'static) -> Rc<Self> {
        Self::build(
            root,
            EditKind::Add(target),
            ItemDraft::default(),
            None,
            close,
        )
    }

    /// The item at `index` of `source`.
    pub(super) fn existing(
        root: PathBuf,
        (index, entry): (usize, &Entry),
        source: &str,
        close: impl Fn() + 'static,
    ) -> Rc<Self> {
        let done = entry.checked == Some(true);
        let draft = ItemDraft::existing(source, entry.range.clone(), done);
        Self::build(root, EditKind::Existing(index), draft, Some(entry), close)
    }

    fn build(
        root: PathBuf,
        kind: EditKind,
        draft: ItemDraft,
        entry: Option<&Entry>,
        close: impl Fn() + 'static,
    ) -> Rc<Self> {
        let target = match &kind {
            EditKind::Add(target) => target.clone(),
            EditKind::Existing(_) => Target::Top,
        };
        let task = entry.is_none_or(|entry| entry.checked.is_some());
        let done = entry.is_some_and(|entry| entry.checked == Some(true));
        let check = gtk::CheckButton::new();
        check.set_tooltip_text(Some("Complete item"));
        check.set_valign(gtk::Align::Center);
        check.set_visible(task);
        check.set_active(done);
        let title = gtk::Entry::new();
        title.set_hexpand(true);
        title.set_placeholder_text(Some("Name the work item"));
        let details = gtk::TextView::new();
        details.set_wrap_mode(gtk::WrapMode::WordChar);
        details.set_top_margin(6);
        details.set_bottom_margin(6);
        details.set_left_margin(8);
        details.set_right_margin(8);
        if let Some(entry) = entry {
            title.set_text(&entry.title);
            details.buffer().set_text(&entry.details);
        }
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&details));
        scroller.set_min_content_height(72);
        scroller.set_has_frame(true);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&check);
        head.append(&title);

        let note = gtk::Label::new(None);
        note.set_xalign(0.0);
        note.set_hexpand(true);
        note.add_css_class("dim-label");
        let count = gtk::Label::new(None);
        let hint = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        hint.append(&note);
        hint.append(&count);
        hint.set_visible(false);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.add_css_class("error");
        error.set_visible(false);

        let close_button = gtk::Button::with_label("Close");
        close_button.set_tooltip_text(Some(&format!(
            "Close \u{b7} {}",
            crate::keys::label("Escape")
        )));
        let delete = gtk::Button::with_label("Delete");
        delete.add_css_class("destructive-action");
        delete.set_tooltip_text(Some(&format!(
            "Delete \u{b7} {}",
            crate::keys::label("<Control>BackSpace")
        )));
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&close_button);
        buttons.append(&delete);

        let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        form.add_css_class("card");
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(form.upcast_ref(), 8);
        }
        form.append(&head);
        form.append(&scroller);
        form.append(&hint);
        form.append(&error);
        form.append(&buttons);

        let this = Rc::new(Self {
            kind,
            form,
            title,
            details,
            check,
            note,
            count,
            hint,
            error,
            root,
            task,
            target,
            draft: RefCell::new(draft),
            landed: RefCell::new(None),
            close: Box::new(close),
        });
        if entry.is_some() {
            *this.landed.borrow_mut() = this.item().ok().map(|item| (item, done));
        }
        this.update_length();
        this.wire(&close_button, &delete);
        this
    }

    fn wire(self: &Rc<Self>, close: &gtk::Button, delete: &gtk::Button) {
        let changed = {
            let weak = Rc::downgrade(self);
            move || {
                if let Some(this) = weak.upgrade() {
                    this.changed();
                }
            }
        };
        let f = changed.clone();
        self.title.connect_changed(move |_| f());
        let f = changed.clone();
        self.details.buffer().connect_changed(move |_| f());
        self.check.connect_toggled(move |_| changed());

        // Return in the title goes on to the points.
        let details = self.details.clone();
        self.title.connect_activate(move |_| {
            details.grab_focus();
        });
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                (this.close)();
            }
        });
        let weak = Rc::downgrade(self);
        delete.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.delete();
            }
        });

        // Esc closes, Ctrl+Backspace deletes, Ctrl+Return closes.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            match key {
                gdk::Key::Escape => (this.close)(),
                gdk::Key::BackSpace if ctrl => this.delete(),
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => (this.close)(),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        self.form.add_controller(keys);
    }

    fn details_text(&self) -> String {
        let buffer = self.details.buffer();
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
    }

    fn item(&self) -> anyhow::Result<WorkItem> {
        WorkItem::new(self.title.text().as_str(), &self.details_text(), self.task)
    }

    fn pending(&self) -> Option<WorkItem> {
        let item = self.item().ok()?;
        let done = self.check.is_active();
        let unchanged = self
            .landed
            .borrow()
            .as_ref()
            .is_some_and(|(landed, was_done)| *landed == item && *was_done == done);
        (!unchanged).then_some(item)
    }

    /// True when nothing typed is waiting to be written.
    fn settled(&self) -> bool {
        let typed = !self.title.text().trim().is_empty() || !self.details_text().trim().is_empty();
        !typed || (self.item().is_ok() && self.pending().is_none())
    }

    fn changed(&self) {
        self.update_length();
        if let Some(item) = self.pending() {
            self.save(&item);
        }
    }

    fn save(&self, item: &WorkItem) {
        let done = self.check.is_active();
        let result = {
            let mut draft = self.draft.borrow_mut();
            draft.set_checked(done);
            if !draft.is_saved() {
                draft.set_target(self.target.clone());
            }
            write_draft(&self.root, &mut draft, &Edit::Save(item))
        };
        match result {
            Ok(()) => {
                *self.landed.borrow_mut() = Some((item.clone(), done));
                self.error.set_visible(false);
                self.update_length();
            }
            Err(error) => self.show_error(&error),
        }
    }

    fn delete(&self) {
        let result = write_draft(&self.root, &mut self.draft.borrow_mut(), &Edit::Remove);
        match result {
            Ok(()) => (self.close)(),
            Err(error) => self.show_error(&error),
        }
    }

    fn show_error(&self, error: &anyhow::Error) {
        self.error.set_text(&error.to_string());
        self.error.set_visible(true);
    }

    /// Countdown near the soft title limit, and a note once past it.
    fn update_length(&self) {
        let left = TITLE_LIMIT as isize - title_length(self.title.text().trim()) as isize;
        self.hint.set_visible(left <= NEAR_LIMIT);
        self.count.set_text(&left.to_string());
        if left < 0 {
            self.count.add_css_class("warning");
        } else {
            self.count.remove_css_class("warning");
        }
        let note = if left >= 0 {
            ""
        } else if self.draft.borrow().is_saved() && self.settled() {
            "Saved \u{b7} long titles wrap in the list"
        } else {
            "Long titles wrap in the list"
        };
        self.note.set_text(note);
    }
}
