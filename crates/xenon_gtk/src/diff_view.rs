//! The diff tab: one file, HEAD against the working tree (or against the
//! index when the two match), read only. `diff_text` loads the texts off the
//! main thread and computes the lines of each side with `similar`; this file
//! puts them in read-only GtkSourceViews (one for unified, two for split that
//! share a vertical adjustment).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use serde_json::json;
use sourceview::prelude::*;
use xenon_core::{TabId, WorkspaceId};
use xenon_git::diff::{Layout, LayoutMode};
use xenon_git::status::GitFile;

use crate::app::App;
use crate::diff_text::{self, Doc, Page};
use crate::pane::focus_inside;

mod numbers;
mod paint;

pub(crate) struct DiffView {
    pub root: gtk::Box,
    /// Holds the one or two panes.
    body: gtk::Box,
    /// Shared by the panes, so they scroll together.
    vadj: gtk::Adjustment,
    path_label: gtk::Label,
    sub_label: gtk::Label,
    auto_label: gtk::Label,
    plus: gtk::Label,
    minus: gtk::Label,
    modes: Vec<(LayoutMode, gtk::ToggleButton)>,
    banner: gtk::Label,
    note: gtk::Label,
    app: Weak<App>,
    ws: WorkspaceId,
    dir: PathBuf,
    pub path: String,
    file: RefCell<GitFile>,
    mode: Cell<LayoutMode>,
    layout: Cell<Layout>,
    doc: RefCell<Option<Arc<Doc>>>,
    /// Header line and editor target line of each hunk.
    hunks: RefCell<Vec<(i32, u32)>>,
    current: Cell<usize>,
    line_h: Cell<f64>,
    views: RefCell<Vec<sourceview::View>>,
    /// Bumped per load, so an older read that finishes late is dropped.
    epoch: Cell<u64>,
    /// True while we flip the toggle buttons ourselves.
    busy: Cell<bool>,
}

/// The Auto / Split / Unified buttons and the Read-only pill.
fn mode_buttons() -> (gtk::Box, Vec<(LayoutMode, gtk::ToggleButton)>, gtk::Box) {
    let modes_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    modes_box.add_css_class("linked");
    let mut modes: Vec<(LayoutMode, gtk::ToggleButton)> = Vec::new();
    for mode in [LayoutMode::Auto, LayoutMode::Split, LayoutMode::Unified] {
        let button = gtk::ToggleButton::with_label(mode.label());
        button.set_focusable(false);
        if let Some((_, first)) = modes.first() {
            button.set_group(Some(first));
        }
        modes_box.append(&button);
        modes.push((mode, button));
    }
    let lock = gtk::Image::from_icon_name("changes-prevent-symbolic");
    let ro = gtk::Label::new(Some("Read-only"));
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    pill.add_css_class("xpill");
    pill.add_css_class("caption");
    pill.add_css_class("dim-label");
    pill.append(&lock);
    pill.append(&ro);
    (modes_box, modes, pill)
}

impl DiffView {
    pub(crate) fn new(
        app: &Rc<App>,
        ws: WorkspaceId,
        _tab: TabId,
        dir: PathBuf,
        file: GitFile,
    ) -> Rc<Self> {
        let path_label = gtk::Label::new(None);
        path_label.set_use_markup(true);
        path_label.set_xalign(0.0);
        path_label.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        let sub_label = gtk::Label::new(None);
        sub_label.add_css_class("dim-label");
        sub_label.add_css_class("caption");
        let auto_label = gtk::Label::new(None);
        auto_label.add_css_class("dim-label");
        auto_label.add_css_class("caption");
        let plus = gtk::Label::new(None);
        plus.add_css_class("xk-added");
        let minus = gtk::Label::new(None);
        minus.add_css_class("xk-deleted");

        let (modes_box, modes, pill) = mode_buttons();
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        head.add_css_class("xdiff-head");
        path_label.set_hexpand(true);
        head.append(&path_label);
        head.append(&sub_label);
        head.append(&auto_label);
        head.append(&modes_box);
        let counts = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        counts.append(&plus);
        counts.append(&minus);
        head.append(&counts);
        head.append(&pill);

        let banner = gtk::Label::new(None);
        banner.add_css_class("xdiff-banner");
        banner.set_wrap(true);
        banner.set_xalign(0.0);
        banner.set_visible(false);
        let note = gtk::Label::new(None);
        note.add_css_class("xdiff-note");
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.set_visible(false);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.add_css_class("xdiff-body");
        body.set_vexpand(true);
        body.set_hexpand(true);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("xdiff");
        root.append(&head);
        root.append(&banner);
        root.append(&note);
        root.append(&body);

        let this = Rc::new(Self {
            root,
            body,
            vadj: gtk::Adjustment::default(),
            path_label,
            sub_label,
            auto_label,
            plus,
            minus,
            modes,
            banner,
            note,
            app: Rc::downgrade(app),
            ws,
            dir,
            path: file.path.clone(),
            file: RefCell::new(file),
            mode: Cell::new(LayoutMode::Auto),
            layout: Cell::new(Layout::Unified),
            doc: RefCell::new(None),
            hunks: RefCell::default(),
            current: Cell::new(0),
            line_h: Cell::new(18.0),
            views: RefCell::default(),
            epoch: Cell::new(0),
            busy: Cell::new(false),
        });
        this.set_path_label();
        this.sync_mode_buttons();
        this.wire();
        this.note.set_text("Loading\u{2026}");
        this.note.set_visible(true);
        this.reload(None);
        this
    }

    fn set_path_label(&self) {
        let (dir, name) = match self.path.rsplit_once('/') {
            Some((dir, name)) => (format!("{dir}/"), name.to_string()),
            None => (String::new(), self.path.clone()),
        };
        self.path_label.set_markup(&format!(
            "<span alpha=\"60%\">{}</span><b>{}</b>",
            glib::markup_escape_text(&dir),
            glib::markup_escape_text(&name)
        ));
        self.path_label.set_tooltip_text(Some(&self.path));
    }

    fn wire(self: &Rc<Self>) {
        for (mode, button) in &self.modes {
            let (weak, mode) = (Rc::downgrade(self), *mode);
            button.connect_toggled(move |b| {
                if let Some(this) = weak.upgrade()
                    && b.is_active()
                    && !this.busy.get()
                {
                    this.set_mode(mode);
                }
            });
        }
        // Auto layout follows the width of the pane area.
        let weak = Rc::downgrade(self);
        self.body.add_tick_callback(move |_, _| {
            if let Some(this) = weak.upgrade()
                && this.mode.get() == LayoutMode::Auto
                && this.doc.borrow().is_some()
                && this.resolve() != this.layout.get()
            {
                this.rebuild();
            }
            glib::ControlFlow::Continue
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let name = match key {
                gdk::Key::n | gdk::Key::bracketright => "next-hunk",
                gdk::Key::p | gdk::Key::bracketleft => "prev-hunk",
                gdk::Key::o | gdk::Key::e | gdk::Key::Return | gdk::Key::KP_Enter => "open",
                gdk::Key::q => "close",
                gdk::Key::l => "layout",
                gdk::Key::j => "line-down",
                gdk::Key::k => "line-up",
                gdk::Key::space => "page-down",
                _ => return glib::Propagation::Proceed,
            };
            this.key(name);
            glib::Propagation::Stop
        });
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        self.body.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_enter(move |_| {
            if let Some(app) = weak.upgrade().and_then(|t| t.app.upgrade()) {
                app.publish_diff();
            }
        });
        self.body.add_controller(focus);
    }

    /// A diff command by name (keys and the `diff-key` action).
    pub(crate) fn key(self: &Rc<Self>, name: &str) {
        let adj = &self.vadj;
        match name {
            "next-hunk" => self.step_hunk(1),
            "prev-hunk" => self.step_hunk(-1),
            "open" => self.open_in_editor(),
            "close" => {
                if let Some(app) = self.app.upgrade() {
                    app.close_active_tab();
                }
            }
            "layout" => self.set_mode(self.mode.get().next()),
            "line-down" => adj.set_value(adj.value() + 3.0 * self.line_h.get()),
            "line-up" => adj.set_value(adj.value() - 3.0 * self.line_h.get()),
            "page-down" => adj.set_value(adj.value() + adj.page_size() * 0.9),
            _ => {}
        }
    }

    pub(crate) fn focus(&self) {
        if let Some(view) = self.views.borrow().last() {
            view.grab_focus();
        }
    }

    pub(crate) fn has_focus(&self) -> bool {
        focus_inside(&self.body)
    }

    pub(crate) fn cycle_layout(self: &Rc<Self>) {
        self.set_mode(self.mode.get().next());
    }

    fn set_mode(self: &Rc<Self>, mode: LayoutMode) {
        self.mode.set(mode);
        self.sync_mode_buttons();
        if self.doc.borrow().is_some() {
            self.rebuild();
        }
    }

    fn sync_mode_buttons(&self) {
        self.busy.set(true);
        for (mode, button) in &self.modes {
            button.set_active(*mode == self.mode.get());
        }
        self.busy.set(false);
    }

    fn resolve(&self) -> Layout {
        self.mode.get().resolve(self.body.width() as f32)
    }

    /// Read the diff again, off the main thread. `file` is a fresher status
    /// entry when the caller has one.
    pub(crate) fn reload(self: &Rc<Self>, file: Option<GitFile>) {
        if let Some(file) = file {
            *self.file.borrow_mut() = file;
        }
        let epoch = self.epoch.get() + 1;
        self.epoch.set(epoch);
        let (dir, file) = (self.dir.clone(), self.file.borrow().clone());
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(doc) = gio::spawn_blocking(move || diff_text::load(&dir, &file)).await else {
                return;
            };
            let Some(this) = weak.upgrade() else { return };
            if this.epoch.get() != epoch {
                return;
            }
            *this.doc.borrow_mut() = Some(Arc::new(doc));
            this.rebuild();
        });
    }

    /// Draw the document again (new doc, layout, or theme), keeping the place.
    pub(crate) fn rebuild(self: &Rc<Self>) {
        let Some(doc) = self.doc.borrow().clone() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let keep = self.vadj.value();
        let refocus = self.has_focus();
        let layout = self.resolve();
        self.layout.set(layout);
        let page = doc.build(layout);
        self.fill(&page);
        *self.hunks.borrow_mut() = page.hunks.iter().map(|h| (h.line, h.target)).collect();
        self.current
            .set(self.current.get().min(page.hunks.len().saturating_sub(1)));
        self.header(&doc, layout);
        self.mark_current();
        if refocus {
            self.focus();
        }
        let adj = self.vadj.clone();
        glib::idle_add_local_once(move || adj.set_value(keep));
        app.publish_diff();
    }

    fn step_hunk(self: &Rc<Self>, delta: isize) {
        let n = self.hunks.borrow().len();
        if n == 0 {
            return;
        }
        let next = (self.current.get() as isize + delta).clamp(0, n as isize - 1) as usize;
        self.current.set(next);
        self.mark_current();
        self.scroll_to_hunk(next);
        if let Some(app) = self.app.upgrade() {
            app.publish_diff();
        }
    }

    fn scroll_to_hunk(&self, index: usize) {
        let Some(&(line, _)) = self.hunks.borrow().get(index) else {
            return;
        };
        if let Some(view) = self.views.borrow().last()
            && let Some(iter) = view.buffer().iter_at_line(line)
        {
            view.scroll_to_iter(&mut iter.clone(), 0.0, true, 0.0, 0.2);
        }
    }

    fn open_in_editor(&self) {
        let target = self
            .hunks
            .borrow()
            .get(self.current.get())
            .map_or(1, |h| h.1);
        if let Some(app) = self.app.upgrade() {
            app.open_file_at(self.ws, &self.dir.join(&self.path), target);
        }
    }

    pub(crate) fn snapshot(&self) -> String {
        let doc = self.doc.borrow();
        json!({
            "path": self.path,
            "mode": self.mode.get().label(),
            "layout": if self.layout.get() == Layout::Split { "split" } else { "unified" },
            "loaded": doc.is_some(),
            "subtitle": doc.as_ref().map(|d| d.subtitle.clone()),
            "added": doc.as_ref().map_or(0, |d| d.added),
            "removed": doc.as_ref().map_or(0, |d| d.removed),
            "hunks": self.hunks.borrow().len(),
            "current": self.current.get(),
            "targets": self.hunks.borrow().iter().map(|h| h.1).collect::<Vec<_>>(),
            "scroll": self.vadj.value(),
            "focused": self.has_focus(),
        })
        .to_string()
    }
}
