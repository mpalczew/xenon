use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_core::WorkspaceId;

use xenon_worklist::{
    Effect, Item, ItemKey, ItemText, Op, Place, TITLE_LIMIT, apply, title_length,
};

use super::fields::{text_of, title_view};
use crate::app::{App, defer};

const NEAR_LIMIT: isize = 12;

const MARGINS: [fn(&gtk::Widget, i32); 4] = [
    gtk::Widget::set_margin_top,
    gtk::Widget::set_margin_bottom,
    gtk::Widget::set_margin_start,
    gtk::Widget::set_margin_end,
];

fn buttons(body: &gtk::Box, editing: bool) -> (gtk::Button, gtk::Button) {
    let delete = gtk::Button::with_label("Delete");
    delete.add_css_class("destructive-action");
    delete.set_visible(editing);
    let save = gtk::Button::with_label("Save");
    save.add_css_class("suggested-action");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&delete);
    row.append(&spacer);
    row.append(&save);
    body.append(&row);
    (delete, save)
}

fn points_view(body: &gtk::Box) -> gtk::TextView {
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
    details
}

fn prefill(item: &Item, check: &gtk::CheckButton, title: &gtk::TextView, details: &gtk::TextView) {
    check.set_active(item.done == Some(true));
    check.set_visible(item.done.is_some());
    title.buffer().set_text(&item.title);
    details.buffer().set_text(&item.details);
}

pub(super) struct Capture {
    app: Weak<App>,
    ws: WorkspaceId,
    root: std::path::PathBuf,
    pub(super) dialog: adw::Dialog,
    pub(super) title: gtk::TextView,
    details: gtk::TextView,
    check: gtk::CheckButton,
    note: gtk::Label,
    count: gtk::Label,
    error: gtk::Label,
    section: gtk::Label,
    hint: gtk::Label,
    prev: gtk::Button,
    next: gtk::Button,
    targets: Vec<(Place, String)>,
    selected: Cell<usize>,
    task: bool,
    editing: bool,
    key: RefCell<Option<ItemKey>>,
    landed: RefCell<Option<ItemText>>,
    discarded: Cell<bool>,
}

impl Capture {
    pub(super) fn build(
        app: &Rc<App>,
        (ws, root): (WorkspaceId, std::path::PathBuf),
        name: &str,
        (targets, selected): (Vec<(Place, String)>, usize),
        edit: Option<Item>,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::new();
        dialog.set_content_width(430);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        for set in MARGINS {
            set(body.upcast_ref(), 12);
        }
        let check = gtk::CheckButton::new();
        check.set_tooltip_text(Some("Complete item"));
        let (title, title_box) = title_view(&format!("Task for {name}"));
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&check);
        head.append(&title_box);
        body.append(&head);
        let details = points_view(&body);

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
        chip.set_visible(targets.iter().any(|(place, _)| *place != Place::Top));
        body.append(&chip);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.add_css_class("error");
        error.set_visible(false);
        body.append(&error);
        dialog.set_child(Some(&body));
        crate::palette_list::close_on_outside_click(&dialog, &body);

        let editing = edit.is_some();
        let (key, task) = match edit {
            Some(item) => {
                chip.set_visible(false);
                prefill(&item, &check, &title, &details);
                (Some(item.key), item.done.is_some())
            }
            None => (None, true),
        };
        let (delete, save) = buttons(&body, editing);

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
            task,
            editing,
            key: RefCell::new(key),
            landed: RefCell::new(None),
            discarded: Cell::new(false),
        });
        if editing {
            *this.landed.borrow_mut() = this.item().ok();
        }
        this.update_section();
        this.wire(&body, &length);
        this.wire_buttons(&save, &delete);
        this
    }

    fn wire_buttons(self: &Rc<Self>, save: &gtk::Button, delete: &gtk::Button) {
        let close = Rc::downgrade(self);
        save.connect_clicked(move |_| {
            defer(&close, move |this| {
                this.dialog.close();
            });
        });
        let weak = Rc::downgrade(self);
        delete.connect_clicked(move |_| {
            defer(&weak, move |this| {
                this.discard();
            });
        });
    }

    fn wire(self: &Rc<Self>, body: &gtk::Box, length: &gtk::Box) {
        let weak = Rc::downgrade(self);
        macro_rules! on_change {
            () => {{
                let (weak, length) = (weak.clone(), length.clone());
                move |_| {
                    let (weak, length) = (weak.clone(), length.clone());
                    defer(&weak, move |this| {
                        this.changed(&length);
                    });
                }
            }};
        }
        self.title.buffer().connect_changed(on_change!());
        self.details.buffer().connect_changed(on_change!());
        self.check.connect_toggled(on_change!());

        let details = self.details.clone();
        let enter = gtk::EventControllerKey::new();
        enter.connect_key_pressed(move |_, key, _, state| {
            let plain =
                !state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK);
            if plain && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
                let details = details.clone();
                glib::idle_add_local_once(move || {
                    details.grab_focus();
                });
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.title.add_controller(enter);
        for (button, delta) in [(&self.prev, -1), (&self.next, 1)] {
            button.set_focusable(false);
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                defer(&weak, move |this| this.cycle(delta));
            });
        }

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
                gdk::Key::Up if alt => {
                    glib::idle_add_local_once(move || this.cycle(-1));
                }
                gdk::Key::Down if alt => {
                    glib::idle_add_local_once(move || this.cycle(1));
                }
                gdk::Key::BackSpace if ctrl => {
                    glib::idle_add_local_once(move || this.discard());
                }
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => {
                    glib::idle_add_local_once(move || {
                        this.dialog.close();
                    });
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        body.add_controller(keys);

        let weak = Rc::downgrade(self);
        self.dialog.connect_closed(move |_| {
            defer(&weak, move |this| this.closed());
        });
    }

    fn target(&self) -> (Place, &str) {
        match self.targets.get(self.selected.get()) {
            Some((place, label)) => (place.clone(), label),
            None => (Place::Top, "Top"),
        }
    }

    fn locked(&self) -> bool {
        self.key.borrow().is_some()
    }

    fn details_text(&self) -> String {
        text_of(&self.details)
    }

    fn title_text(&self) -> String {
        text_of(&self.title).replace(['\n', '\r'], " ")
    }

    fn item(&self) -> anyhow::Result<ItemText> {
        let done = self.task.then(|| self.check.is_active());
        ItemText::new(self.title_text().as_str(), &self.details_text(), done)
    }

    fn pending(&self) -> Option<ItemText> {
        let item = self.item().ok()?;
        let unchanged = self.landed.borrow().as_ref().is_some_and(|l| *l == item);
        (!unchanged).then_some(item)
    }

    fn settled(&self) -> bool {
        let typed = !self.title_text().trim().is_empty() || !self.details_text().trim().is_empty();
        !typed || (self.item().is_ok() && self.pending().is_none())
    }

    fn changed(&self, length: &gtk::Box) {
        self.update_length(length);
        if let Some(item) = self.pending() {
            self.save(&item);
        }
    }

    fn save(&self, text: &ItemText) {
        let known = self.key.borrow().clone();
        let adding = known.is_none();
        let op = match known {
            Some(item) => Op::Replace {
                item,
                text: text.clone(),
            },
            None => Op::Add {
                to: self.target().0,
                text: text.clone(),
            },
        };
        let error = match apply(&self.root, op) {
            Ok(applied) => match applied.effect {
                Effect::Saved { item, .. } => {
                    self.key.replace_with(|old| item.or(old.take()));
                    None
                }
                Effect::Unchanged => None,
                Effect::Gone if adding => {
                    Some("That section is gone; reopen to pick another".into())
                }
                Effect::Gone => {
                    Some("This item changed elsewhere; reopen it to keep editing".into())
                }
            },
            Err(error) => Some(error.to_string()),
        };
        if let Some(error) = error {
            self.error.set_text(&error);
            self.error.set_visible(true);
            return;
        }
        *self.landed.borrow_mut() = Some(text.clone());
        self.error.set_visible(false);
        if let Some(app) = self.app.upgrade() {
            app.worklist
                .last_section
                .borrow_mut()
                .insert(self.ws, self.target().0);
        }
        self.update_section();
    }

    fn update_length(&self, length: &gtk::Box) {
        let left = TITLE_LIMIT as isize - title_length(self.title_text().trim()) as isize;
        length.set_visible(left <= NEAR_LIMIT);
        self.count.set_text(&left.to_string());
        if left < 0 {
            self.count.add_css_class("warning");
        } else {
            self.count.remove_css_class("warning");
        }
        let note = if left >= 0 {
            ""
        } else if self.locked() && self.settled() {
            "Saved \u{b7} long titles wrap in the list"
        } else {
            "Long titles wrap in the list"
        };
        self.note.set_text(note);
    }

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
                .insert(self.ws, self.target().0);
        }
        self.update_section();
    }

    fn update_section(&self) {
        let locked = self.locked();
        self.section.set_text(self.target().1);
        self.prev.set_sensitive(!locked);
        self.next.set_sensitive(!locked);
        self.hint.set_text(if locked {
            "Saved here"
        } else {
            "Alt+\u{2191} Alt+\u{2193} section"
        });
    }

    fn discard(&self) {
        self.discarded.set(true);
        let item = self.key.borrow_mut().take();
        if let (Some(item), Some(app)) = (item, self.app.upgrade()) {
            match apply(&self.root, Op::Remove { item }) {
                Ok(applied) => {
                    if let (Effect::Saved { undo, .. }, true) = (applied.effect, self.editing) {
                        app.offer_worklist_undo(self.root.clone(), "Item deleted", undo);
                    }
                }
                Err(error) => app.worklist_error("Couldn\u{2019}t delete that task", &error),
            }
        }
        self.dialog.close();
    }

    fn closed(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.worklist.capture.borrow_mut().take();
        if self.editing || self.discarded.get() || !self.settled() || !self.locked() {
            return;
        }
        let item = self.key.borrow().clone();
        if let Some(item) = item {
            app.task_added_toast(self.ws, item, self.title_text().trim());
        }
    }
}
