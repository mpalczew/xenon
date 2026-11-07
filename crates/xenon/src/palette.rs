//! The one palette shell the finders and pickers share: an `adw::Dialog` with
//! a search field and a list of rows in stock style classes. It
//! draws what a model hands it and reports keys back; ranking, discovery and
//! what Enter does live in the models (`finder.rs`, `picker.rs`).
//!
//! Focus stays in the field. Up/Down, Ctrl+N/Ctrl+P (and Ctrl+J/K) move,
//! Enter accepts, Tab completes, Escape closes and focus returns to the widget
//! that had it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use crate::palette_list::{close_on_outside_click, row_list};

/// One row, as the model wants it drawn.
#[derive(Clone, Default)]
pub(crate) struct PaletteRow {
    pub title: String,
    /// Char indexes of `title` to emphasise (the query's matched letters).
    pub hits: Vec<u32>,
    pub detail: Option<String>,
    pub badge: Option<String>,
    pub enabled: bool,
}

/// What only some palettes need; the default suits a plain list.
#[derive(Default)]
pub(crate) struct Options {
    /// A line above the field: the question of an input prompt.
    pub prompt: Option<String>,
    /// Text the field starts with.
    pub initial: String,
    /// Hide what is typed (password inputs).
    pub masked: bool,
    /// Enter answers with the field text even when no row is selected.
    pub free_text: bool,
    /// Escape or closing the dialog; a close by code does not call it.
    pub on_dismiss: Option<Box<dyn Fn()>>,
    /// Ctrl+Enter on the selected row.
    pub on_accept_alt: Option<Box<dyn Fn(usize)>>,
    /// A button after the hint, with the text it shows, and what it does.
    /// Its shortcut is the model's business (it calls `Palette::run_action`).
    pub action: Option<(String, Box<dyn Fn()>)>,
}

/// What the models answer to.
pub(crate) struct Handlers {
    pub placeholder: String,
    pub hint: String,
    pub options: Options,
    pub on_query: Box<dyn Fn(&str)>,
    pub on_accept: Box<dyn Fn(usize)>,
    /// Tab: complete the selected row into the field.
    pub on_complete: Option<Box<dyn Fn(usize)>>,
}

pub(crate) struct Palette {
    dialog: adw::Dialog,
    entry: gtk::SearchEntry,
    prompt: gtk::Label,
    hint: gtk::Label,
    action: gtk::Button,
    empty: gtk::Label,
    scroller: gtk::ScrolledWindow,
    store: gio::ListStore,
    selection: gtk::SingleSelection,
    view: gtk::ListView,
    restore: RefCell<Option<gtk::Widget>>,
    /// True while focus goes back to where it was; the app ignores that move.
    pub(crate) restoring: Cell<bool>,
    handlers: RefCell<Option<Rc<Handlers>>>,
    open: Cell<bool>,
    /// Counts openings, so late answers for an earlier opening can be dropped.
    epoch: Cell<u64>,
    on_change: RefCell<Option<Box<dyn Fn()>>>,
    titles: RefCell<Vec<String>>,
    /// Row count and which are enabled, for stepping over disabled rows.
    enabled: RefCell<Vec<bool>>,
}

/// A label with stock style classes and the margins the dialog's content uses.
fn text_label(classes: &[&str]) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_css_classes(classes);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label
}

impl Palette {
    pub(crate) fn new() -> Rc<Self> {
        let entry = gtk::SearchEntry::new();
        entry.set_margin_top(12);
        entry.set_margin_bottom(6);
        entry.set_margin_start(12);
        entry.set_margin_end(12);
        let (store, selection, view) = row_list();

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&view));
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_max_content_height(380);
        scroller.set_propagate_natural_height(true);
        scroller.set_focusable(false);

        let prompt = text_label(&["heading"]);
        prompt.set_margin_top(12);
        let empty = text_label(&["dim-label"]);
        empty.set_margin_top(12);
        empty.set_margin_bottom(12);
        let hint = text_label(&["caption", "dim-label"]);
        hint.set_hexpand(true);
        let action = gtk::Button::new();
        action.add_css_class("flat");
        action.set_focusable(false);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        footer.append(&hint);
        footer.append(&action);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_size_request(640, -1);
        content.append(&prompt);
        content.append(&entry);
        content.append(&scroller);
        content.append(&empty);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&footer);

        let dialog = adw::Dialog::new();
        dialog.set_follows_content_size(true);
        dialog.set_child(Some(&content));
        dialog.set_focus(Some(&entry));
        close_on_outside_click(&dialog, &content);

        let this = Rc::new(Self {
            dialog,
            entry,
            prompt,
            hint,
            action,
            empty,
            scroller,
            store,
            selection,
            view,
            restore: RefCell::new(None),
            restoring: Cell::new(false),
            handlers: RefCell::new(None),
            open: Cell::new(false),
            epoch: Cell::new(0),
            on_change: RefCell::new(None),
            titles: RefCell::default(),
            enabled: RefCell::default(),
        });
        this.wire(Rc::downgrade(&this));
        this
    }

    fn wire(&self, weak: std::rc::Weak<Self>) {
        // Escape (the dialog's own), or any close not made by code: a cancel.
        let w = weak.clone();
        self.dialog.connect_closed(move |_| {
            if let Some(this) = w.upgrade() {
                this.dismiss();
            }
        });
        let w = weak.clone();
        self.action.connect_clicked(move |_| {
            if let Some(this) = w.upgrade() {
                this.run_action();
            }
        });
        let w = weak.clone();
        self.entry.connect_changed(move |entry| {
            let Some(this) = w.upgrade() else { return };
            if !this.open.get() {
                return;
            }
            let handlers = this.handlers.borrow().clone();
            if let Some(h) = handlers {
                (h.on_query)(entry.text().as_str());
            }
        });
        let w = weak.clone();
        self.entry.connect_activate(move |_| {
            if let Some(this) = w.upgrade() {
                this.accept();
            }
        });
        // A row click accepts it.
        let w = weak.clone();
        self.view.connect_activate(move |_, position| {
            if let Some(this) = w.upgrade() {
                this.selection.set_selected(position);
                this.accept();
            }
        });

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let w = weak;
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = w.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            let handled = match key {
                gdk::Key::Escape => {
                    this.dismiss();
                    true
                }
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => {
                    this.accept_alt();
                    true
                }
                gdk::Key::Down => this.step(1),
                gdk::Key::Up => this.step(-1),
                gdk::Key::Page_Down => this.step(8),
                gdk::Key::Page_Up => this.step(-8),
                gdk::Key::Tab | gdk::Key::ISO_Left_Tab => this.complete(),
                gdk::Key::n | gdk::Key::j if ctrl => this.step(1),
                gdk::Key::p | gdk::Key::k if ctrl => this.step(-1),
                _ => false,
            };
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.entry.add_controller(keys);
    }

    /// Called after anything an observer might show changes (open, close, rows,
    /// selection).
    pub(crate) fn set_on_change(&self, f: impl Fn() + 'static) {
        *self.on_change.borrow_mut() = Some(Box::new(f));
    }

    fn changed(&self) {
        if let Some(f) = self.on_change.borrow().as_ref() {
            f();
        }
    }

    /// JSON picture of the palette for scripts (`app.palette-state`).
    pub(crate) fn snapshot(&self) -> String {
        let titles = self.titles.borrow();
        serde_json::json!({
            "open": self.open.get(),
            "query": self.entry.text().as_str(),
            "rows": titles.iter().take(12).collect::<Vec<_>>(),
            "count": titles.len(),
            "selected": self.selection.selected(),
            "selected_title": titles.get(self.selection.selected() as usize),
            "entry_focused": crate::pane::focus_inside(&self.entry),
        })
        .to_string()
    }

    /// A key by name, as the entry's own key handling would treat it.
    pub(crate) fn key(&self, name: &str) {
        match name {
            "escape" => self.dismiss(),
            "enter" => self.accept(),
            "ctrl-enter" => self.accept_alt(),
            "tab" => {
                self.complete();
            }
            "down" | "ctrl-n" => {
                self.step(1);
            }
            "up" | "ctrl-p" => {
                self.step(-1);
            }
            _ => {}
        }
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.get()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Show the palette. `win` supplies the focus to give back on close.
    pub(crate) fn open(&self, win: &impl IsA<gtk::Window>, handlers: Handlers) {
        let placeholder = handlers.placeholder.clone();
        let (hint, initial) = (handlers.hint.clone(), handlers.options.initial.clone());
        let initial = initial.as_str();
        if let Some(text) = self.entry.delegate().and_downcast::<gtk::Text>() {
            text.set_visibility(!handlers.options.masked);
        }
        let action = handlers
            .options
            .action
            .as_ref()
            .map(|(text, _)| text.clone());
        self.action.set_label(action.as_deref().unwrap_or(""));
        self.action.set_visible(action.is_some());
        self.prompt
            .set_text(handlers.options.prompt.as_deref().unwrap_or(""));
        self.prompt.set_visible(handlers.options.prompt.is_some());
        if !self.open.get() {
            *self.restore.borrow_mut() = win.focus();
        }
        let was_open = self.open.replace(true);
        self.epoch.set(self.epoch.get() + 1);
        *self.handlers.borrow_mut() = Some(Rc::new(handlers));
        self.entry.set_placeholder_text(Some(&placeholder));
        self.hint.set_text(&hint);
        self.entry.set_text(initial);
        self.entry.set_position(-1);
        if !was_open {
            self.dialog.present(Some(win.as_ref()));
        }
        if let Some(text) = self.entry.delegate().and_downcast::<gtk::Text>() {
            text.grab_focus_without_selecting();
        }
        self.changed();
        // Paint the first rows for the initial query.
        let handlers = self.handlers.borrow().clone();
        if let Some(h) = handlers {
            (h.on_query)(initial);
        }
    }

    /// Escape or closing the dialog: close, then tell the model it was a cancel.
    fn dismiss(&self) {
        let handlers = self.handlers.borrow().clone();
        if !self.open.get() {
            return;
        }
        self.close();
        if let Some(cancel) = handlers
            .as_ref()
            .and_then(|h| h.options.on_dismiss.as_ref())
        {
            cancel();
        }
    }

    pub(crate) fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.dialog.force_close();
        *self.handlers.borrow_mut() = None;
        self.store.remove_all();
        self.titles.borrow_mut().clear();
        if let Some(widget) = self.restore.borrow_mut().take() {
            self.restoring.set(true);
            widget.grab_focus();
            self.restoring.set(false);
        }
        self.changed();
    }

    /// Run the options' button action, if the open palette has one.
    pub(crate) fn run_action(&self) -> bool {
        let handlers = self.handlers.borrow().clone();
        let action = handlers.as_ref().and_then(|h| h.options.action.as_ref());
        action.map(|(_, run)| run()).is_some()
    }

    pub(crate) fn text(&self) -> String {
        self.entry.text().to_string()
    }

    /// Replace the field text (completion) and put the cursor at the end.
    pub(crate) fn set_text(&self, text: &str) {
        self.entry.set_text(text);
        self.entry.set_position(-1);
    }

    /// Show `rows`; the first enabled row is selected.
    pub(crate) fn set_rows(&self, rows: Vec<PaletteRow>, empty_text: &str) {
        *self.enabled.borrow_mut() = rows.iter().map(|r| r.enabled).collect();
        *self.titles.borrow_mut() = rows.iter().map(|r| r.title.clone()).collect();
        let first = rows.iter().position(|r| r.enabled).unwrap_or(0);
        let items: Vec<glib::BoxedAnyObject> =
            rows.into_iter().map(glib::BoxedAnyObject::new).collect();
        let has_rows = !items.is_empty();
        self.store.splice(0, self.store.n_items(), &items);
        self.empty.set_text(empty_text);
        self.empty.set_visible(!has_rows && !empty_text.is_empty());
        self.scroller.set_visible(has_rows);
        if has_rows {
            self.select(first as u32);
        }
        self.changed();
    }

    pub(crate) fn select(&self, index: u32) {
        self.selection.set_selected(index);
        self.view.scroll_to(index, gtk::ListScrollFlags::NONE, None);
        self.changed();
    }

    /// Move the selection by `delta` enabled rows. Always true (key handled).
    pub(crate) fn step(&self, delta: i32) -> bool {
        let enabled = self.enabled.borrow().clone();
        let count = enabled.len() as i32;
        if count == 0 {
            return true;
        }
        let mut at = self.selection.selected() as i32;
        let dir = delta.signum();
        for _ in 0..delta.abs() {
            let mut next = at;
            loop {
                next += dir;
                if next < 0 || next >= count {
                    next = at;
                    break;
                }
                if enabled[next as usize] {
                    break;
                }
            }
            at = next;
        }
        self.select(at as u32);
        true
    }

    fn accept(&self) {
        let handlers = self.handlers.borrow().clone();
        let at = self.selection.selected();
        let ok = self
            .enabled
            .borrow()
            .get(at as usize)
            .copied()
            .unwrap_or(false)
            || handlers.as_ref().is_some_and(|h| h.options.free_text);
        if let (Some(h), true) = (handlers, ok) {
            (h.on_accept)(at as usize);
        }
    }

    fn accept_alt(&self) {
        let handlers = self.handlers.borrow().clone();
        let at = self.selection.selected() as usize;
        match handlers
            .as_ref()
            .and_then(|h| h.options.on_accept_alt.as_ref())
        {
            Some(alt) if self.enabled.borrow().get(at).copied().unwrap_or(false) => alt(at),
            Some(_) => {}
            None => self.accept(),
        }
    }

    fn complete(&self) -> bool {
        let handlers = self.handlers.borrow().clone();
        let at = self.selection.selected() as usize;
        match handlers.as_ref().and_then(|h| h.on_complete.as_ref()) {
            Some(complete) => {
                complete(at);
                true
            }
            None => false,
        }
    }
}
