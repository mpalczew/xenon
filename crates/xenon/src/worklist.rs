//! The worklist: quick capture into `.xenon/worklist.md` and "Open Worklist".
//!
//! The file model (parse, edit, sections) is `xenon_store::worklist`. Opening
//! shows the file in the list view (`worklist_list.rs`) or, by choice, the
//! Markdown writing mode; capture is a stock `adw::Dialog` that saves as you
//! type, as the GPUI capture did.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_core::WorkspaceId;
use xenon_store::worklist::{
    Edit, ItemDraft, TITLE_LIMIT, Target, WorkItem, WorklistFile, capture_targets, read_source,
    title_length, write_draft,
};

use crate::app::App;

/// Characters left at which the countdown appears.
const NEAR_LIMIT: isize = 12;

/// Capture state that outlives one dialog.
#[derive(Default)]
pub(crate) struct Worklist {
    capture: RefCell<Option<Rc<Capture>>>,
    /// The section new text last went into, per workspace.
    last_section: RefCell<HashMap<WorkspaceId, Target>>,
    /// The item the showing toast can take back, and which toast that is.
    undo: RefCell<Option<(WorkspaceId, ItemDraft)>>,
    toast_serial: Cell<u64>,
}

impl App {
    fn workspace_for_worklist(&self) -> Option<(WorkspaceId, std::path::PathBuf, String)> {
        let model = self.model.borrow();
        let ws = model.active()?;
        let rec = model.rec(ws)?;
        Some((ws, rec.root.clone(), rec.name.clone()))
    }

    /// Open Worklist: `.xenon/worklist.md` in the list view, made when missing.
    pub(crate) fn open_worklist(self: &Rc<Self>) {
        let Some((ws, root, _)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let capture = self.worklist.capture.borrow().clone();
        if let Some(capture) = capture {
            capture.dialog.close();
        }
        let made = WorklistFile::new(&root).and_then(|file| match file.read()? {
            Some(_) => Ok(()),
            None => file.write(None, b"# Worklist\n"),
        });
        if let Err(error) = made {
            self.toast_failed("Couldn\u{2019}t open the worklist", &error.to_string());
            return;
        }
        self.open_editor(ws, &root.join(".xenon/worklist.md"));
    }

    /// Add to Worklist: the capture dialog; the same key again closes it.
    pub(crate) fn capture_worklist(self: &Rc<Self>) {
        let open = self.worklist.capture.borrow().clone();
        if let Some(open) = open {
            open.dialog.close();
            return;
        }
        let Some((ws, root, name)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let targets = capture_targets(&read_source(&root));
        let preferred = self.worklist.last_section.borrow().get(&ws).cloned();
        let selected = preferred
            .and_then(|want| targets.iter().position(|t| t.label() == want.label()))
            .unwrap_or(0);
        let capture = Capture::build(self, (ws, root), &name, targets, selected);
        *self.worklist.capture.borrow_mut() = Some(capture.clone());
        capture.dialog.present(Some(&self.win));
        capture.title.grab_focus();
    }

    /// Take back the capture the toast offers.
    pub(crate) fn undo_capture(self: &Rc<Self>) {
        let Some((ws, mut draft)) = self.worklist.undo.borrow_mut().take() else {
            return;
        };
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        match write_draft(&root, &mut draft, &Edit::Remove) {
            Ok(()) => self.toast("Poof. Capture undone."),
            Err(error) => self.worklist_error("Can\u{2019}t undo just yet", &error),
        }
    }

    fn worklist_error(&self, title: &str, error: &anyhow::Error) {
        let toast = adw::Toast::new(&format!("{title} \u{2014} {error}"));
        toast.set_use_markup(false);
        toast.set_button_label(Some("Show worklist"));
        toast.set_action_name(Some("app.open-worklist"));
        self.toasts.add_toast(toast);
    }

    fn task_added_toast(self: &Rc<Self>, ws: WorkspaceId, draft: ItemDraft) {
        let text = match draft.title() {
            Some(title) => format!("Got it, task added \u{2014} {title}"),
            None => "Got it, task added".to_string(),
        };
        let toast = adw::Toast::new(&text);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        toast.set_action_name(Some("app.undo-capture"));
        let serial = self.worklist.toast_serial.get() + 1;
        self.worklist.toast_serial.set(serial);
        *self.worklist.undo.borrow_mut() = Some((ws, draft));
        let weak = Rc::downgrade(self);
        toast.connect_dismissed(move |_| {
            if let Some(app) = weak.upgrade()
                && app.worklist.toast_serial.get() == serial
            {
                app.worklist.undo.borrow_mut().take();
            }
        });
        self.toasts.add_toast(toast);
    }
}

/// One open capture dialog.
struct Capture {
    app: Weak<App>,
    ws: WorkspaceId,
    root: std::path::PathBuf,
    dialog: adw::Dialog,
    title: gtk::Entry,
    details: gtk::TextView,
    check: gtk::CheckButton,
    note: gtk::Label,
    count: gtk::Label,
    error: gtk::Label,
    section: gtk::Label,
    hint: gtk::Label,
    prev: gtk::Button,
    next: gtk::Button,
    targets: Vec<Target>,
    selected: Cell<usize>,
    draft: RefCell<ItemDraft>,
    /// The item and done state as last written.
    landed: RefCell<Option<(WorkItem, bool)>>,
    discarded: Cell<bool>,
}

impl Capture {
    fn build(
        app: &Rc<App>,
        (ws, root): (WorkspaceId, std::path::PathBuf),
        name: &str,
        targets: Vec<Target>,
        selected: usize,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::new();
        dialog.set_content_width(430);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(body.upcast_ref(), 12);
        }
        let check = gtk::CheckButton::new();
        check.set_tooltip_text(Some("Complete item"));
        let title = gtk::Entry::new();
        title.set_hexpand(true);
        title.set_placeholder_text(Some(&format!("Task for {name}")));
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&check);
        head.append(&title);
        body.append(&head);
        let details = gtk::TextView::new();
        details.set_wrap_mode(gtk::WrapMode::WordChar);
        details.set_top_margin(6);
        details.set_bottom_margin(6);
        details.set_left_margin(8);
        details.set_right_margin(8);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&details));
        scroller.set_min_content_height(88);
        scroller.set_has_frame(true);
        body.append(&scroller);

        let note = gtk::Label::new(None);
        note.set_xalign(0.0);
        note.set_hexpand(true);
        note.add_css_class("dim-label");
        let count = gtk::Label::new(None);
        let length = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        length.append(&note);
        length.append(&count);
        length.set_visible(false);
        body.append(&length);

        let section = gtk::Label::new(None);
        section.set_width_chars(10);
        let prev = gtk::Button::with_label("\u{2039}");
        let next = gtk::Button::with_label("\u{203a}");
        let hint = gtk::Label::new(None);
        hint.add_css_class("dim-label");
        hint.set_hexpand(true);
        hint.set_xalign(1.0);
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chip.append(&gtk::Label::new(Some("Into")));
        chip.append(&prev);
        chip.append(&section);
        chip.append(&next);
        chip.append(&hint);
        chip.set_visible(targets.iter().any(Target::is_section));
        body.append(&chip);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.add_css_class("error");
        error.set_visible(false);
        body.append(&error);
        dialog.set_child(Some(&body));
        crate::palette_list::close_on_outside_click(&dialog, &body);

        let this = Rc::new(Self {
            app: Rc::downgrade(app),
            ws,
            root,
            dialog,
            title,
            details,
            check,
            note,
            count,
            error,
            section,
            hint,
            prev,
            next,
            targets,
            selected: Cell::new(selected),
            draft: RefCell::default(),
            landed: RefCell::new(None),
            discarded: Cell::new(false),
        });
        this.update_section();
        this.wire(&body, &length);
        this
    }

    fn wire(self: &Rc<Self>, body: &gtk::Box, length: &gtk::Box) {
        let weak = Rc::downgrade(self);
        let changed = {
            let length = length.clone();
            move || {
                if let Some(this) = weak.upgrade() {
                    this.changed(&length);
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
        for (button, delta) in [(&self.prev, -1), (&self.next, 1)] {
            button.set_focusable(false);
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.cycle(delta);
                }
            });
        }

        // Alt+Up/Down section, Ctrl+Backspace delete, Ctrl+Return close.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let alt = state.contains(gdk::ModifierType::ALT_MASK);
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            match key {
                gdk::Key::Up if alt => this.cycle(-1),
                gdk::Key::Down if alt => this.cycle(1),
                gdk::Key::BackSpace if ctrl => this.discard(),
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => {
                    this.dialog.close();
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        body.add_controller(keys);

        let weak = Rc::downgrade(self);
        self.dialog.connect_closed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.closed();
            }
        });
    }

    fn target(&self) -> Target {
        self.targets
            .get(self.selected.get())
            .cloned()
            .unwrap_or_default()
    }

    /// Once saved, the item stays where it landed; moving it is a Markdown edit.
    fn locked(&self) -> bool {
        self.draft.borrow().is_saved()
    }

    fn details_text(&self) -> String {
        let buffer = self.details.buffer();
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
    }

    fn item(&self) -> anyhow::Result<WorkItem> {
        WorkItem::new(self.title.text().as_str(), &self.details_text(), true)
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

    fn changed(&self, length: &gtk::Box) {
        self.update_length(length);
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
                draft.set_target(self.target());
            }
            write_draft(&self.root, &mut draft, &Edit::Save(item))
        };
        match result {
            Ok(()) => {
                *self.landed.borrow_mut() = Some((item.clone(), done));
                self.error.set_visible(false);
                if let Some(app) = self.app.upgrade() {
                    app.worklist
                        .last_section
                        .borrow_mut()
                        .insert(self.ws, self.target());
                }
                self.update_section();
            }
            Err(error) => {
                self.error.set_text(&error.to_string());
                self.error.set_visible(true);
            }
        }
    }

    /// Countdown near the soft title limit, and a note once past it.
    fn update_length(&self, length: &gtk::Box) {
        let left = TITLE_LIMIT as isize - title_length(self.title.text().trim()) as isize;
        length.set_visible(left <= NEAR_LIMIT);
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

    /// "Into ‹ Section ›": cycles with Alt+Up/Down or the arrows.
    fn cycle(&self, delta: isize) {
        let count = self.targets.len() as isize;
        if count < 2 || self.locked() {
            return;
        }
        self.selected
            .set((self.selected.get() as isize + delta).rem_euclid(count) as usize);
        if let Some(app) = self.app.upgrade() {
            app.worklist
                .last_section
                .borrow_mut()
                .insert(self.ws, self.target());
        }
        self.update_section();
    }

    fn update_section(&self) {
        let locked = self.locked();
        self.section.set_text(self.target().label());
        self.prev.set_sensitive(!locked);
        self.next.set_sensitive(!locked);
        self.hint.set_text(if locked {
            "Saved here"
        } else {
            "Alt+\u{2191} Alt+\u{2193} section"
        });
    }

    /// Ctrl+Backspace: drop the item and close.
    fn discard(&self) {
        self.discarded.set(true);
        let result = write_draft(&self.root, &mut self.draft.borrow_mut(), &Edit::Remove);
        if let (Err(error), Some(app)) = (result, self.app.upgrade()) {
            app.worklist_error("Couldn\u{2019}t delete that task", &error);
        }
        self.dialog.close();
    }

    /// A fully saved item gets an Undo notice.
    fn closed(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.worklist.capture.borrow_mut().take();
        if self.discarded.get() || !self.settled() || !self.locked() {
            return;
        }
        let draft = self.draft.borrow().clone();
        app.task_added_toast(self.ws, draft);
    }
}
