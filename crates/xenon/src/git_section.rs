//! The Git section of the sidebar: branch and count in the header, one row per
//! changed file (status badges, colored name, muted folder, five-cell change
//! bar), or just the header with Clean when there are none. Rows come from `xenon_git`'s status; this
//! file only makes widgets and moves the keyboard.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_core::WorkspaceId;
use xenon_git::status::{GitFile, GitStatus};

use crate::app::App;
use crate::git_rows::{make_row, more_row};
use crate::model::Fold;
use crate::pane::focus_inside;
use crate::sidebar_ui::{Row, Section};

/// Rows drawn at most; a repo with more changes shows the first ones and a
/// note, so a huge dirty tree cannot make the sidebar slow.
const MAX_ROWS: usize = 400;

pub(crate) struct GitView {
    pub root: gtk::Box,
    section: Section,
    clean: gtk::Box,
    branch: gtk::Label,
    count: gtk::Label,
    list: gtk::ListBox,
    app: Weak<App>,
    ws: Cell<Option<WorkspaceId>>,
    status: RefCell<Option<GitStatus>>,
    shown: RefCell<Option<String>>,
    /// Workspace folder, for the full paths in row hints.
    dir: RefCell<std::path::PathBuf>,
    /// Files in row order (the rows may be capped).
    files: RefCell<Vec<GitFile>>,
}

impl GitView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let branch = gtk::Label::new(None);
        branch.add_css_class("dim-label");
        branch.add_css_class("caption");
        branch.set_ellipsize(gtk::pango::EllipsizeMode::End);
        branch.set_max_width_chars(16);
        let count = gtk::Label::new(None);
        count.add_css_class("dim-label");
        count.add_css_class("caption");
        count.add_css_class("numeric");
        let clean = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let tick = gtk::Image::from_icon_name("object-select-symbolic");
        tick.add_css_class("success");
        let word = gtk::Label::new(Some("Clean"));
        word.add_css_class("caption");
        clean.append(&tick);
        clean.append(&word);
        clean.set_visible(false);
        let trailing = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        trailing.append(&clean);
        trailing.append(&branch);
        trailing.append(&count);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        let section = Section::new("GIT", Fold::Git, &list, Some(230), &app);
        section.set_trailing(&trailing);
        section.sep.set_visible(false);
        let root = section.root.clone();
        root.set_visible(false);
        let this = Rc::new(Self {
            root,
            section,
            clean,
            branch,
            count,
            list,
            app,
            ws: Cell::new(None),
            status: RefCell::new(None),
            shown: RefCell::new(None),
            dir: RefCell::new(std::path::PathBuf::new()),
            files: RefCell::default(),
        });
        this.wire();
        this
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        // Deferred: rebuilding the list inside its own activate handler crashes GTK.
        self.list.connect_row_activated(move |_, row| {
            let index = row.index().max(0) as usize;
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.open_row(index);
                }
            });
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if key == gdk::Key::Escape
                && !state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK)
            {
                if let Some(app) = this.app.upgrade() {
                    glib::idle_add_local_once(move || app.focus_terminal());
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.root.add_controller(keys);
    }

    /// Draw the fold the model decided.
    pub(crate) fn show_fold(&self, collapsed: bool) {
        self.section.show_fold(collapsed);
    }

    fn open_row(&self, index: usize) {
        let path = self.files.borrow().get(index).map(|f| f.path.clone());
        if let (Some(path), Some(app)) = (path, self.app.upgrade()) {
            app.open_diff(&path);
        }
    }

    /// Show the status of workspace `ws` (nothing when it is not a git
    /// folder). `shown` is the file whose diff tab is on screen.
    pub(crate) fn set(
        &self,
        ws: Option<WorkspaceId>,
        status: Option<&GitStatus>,
        shown: Option<String>,
        dir: &std::path::Path,
    ) {
        let same = self.ws.get() == ws
            && self.status.borrow().as_ref() == status
            && *self.shown.borrow() == shown
            && *self.dir.borrow() == dir;
        if same {
            return;
        }
        let had_focus = focus_inside(&self.root);
        let keep = self.selected_path();
        let rebuild = self.ws.get() != ws
            || self.status.borrow().as_ref() != status
            || *self.dir.borrow() != dir;
        *self.dir.borrow_mut() = dir.to_path_buf();
        self.ws.set(ws);
        *self.status.borrow_mut() = status.cloned();
        *self.shown.borrow_mut() = shown;
        self.root.set_visible(status.is_some());
        let Some(status) = status else {
            self.clear();
            return;
        };
        if rebuild {
            self.rebuild(status, keep, had_focus);
        } else {
            self.mark_shown();
        }
    }

    fn clear(&self) {
        self.list.remove_all();
        self.files.borrow_mut().clear();
    }

    fn rebuild(&self, status: &GitStatus, keep: Option<String>, had_focus: bool) {
        self.clear();
        self.branch.set_text(&status.branch);
        self.count.set_text(&status.files.len().to_string());
        self.count.set_visible(!status.files.is_empty());
        let is_clean = status.files.is_empty();
        self.clean.set_visible(is_clean);
        self.section.set_foldable(!is_clean);
        if is_clean {
            return;
        }
        let shown: Vec<GitFile> = status.files.iter().take(MAX_ROWS).cloned().collect();
        for file in &shown {
            self.list.append(&make_row(file, &self.dir.borrow()));
        }
        if status.files.len() > shown.len() {
            let more = format!("and {} more", status.files.len() - shown.len());
            self.list.append(&more_row(&more));
        }
        *self.files.borrow_mut() = shown;
        self.mark_shown();
        if let Some(path) = keep {
            self.select_path(&path, had_focus);
        }
    }

    fn mark_shown(&self) {
        let shown = self.shown.borrow().clone();
        for (i, file) in self.files.borrow().iter().enumerate() {
            if let Some(row) = self
                .list
                .row_at_index(i as i32)
                .and_then(|r| r.child())
                .and_then(|c| Row::of(&c))
            {
                row.set_open(shown.as_deref() == Some(file.path.as_str()));
            }
        }
    }

    fn selected_path(&self) -> Option<String> {
        let row = self.list.selected_row()?;
        self.files
            .borrow()
            .get(row.index().max(0) as usize)
            .map(|f| f.path.clone())
    }

    fn select_path(&self, path: &str, focus: bool) {
        let index = self.files.borrow().iter().position(|f| f.path == path);
        if let Some(row) = index.and_then(|i| self.list.row_at_index(i as i32)) {
            self.list.select_row(Some(&row));
            if focus {
                row.grab_focus();
            }
        }
    }

    pub(crate) fn set_hint(&self, text: &str) {
        self.section.fold.set_tooltip_text(Some(text));
    }

    pub(crate) fn has_focus(&self) -> bool {
        focus_inside(&self.root)
    }

    /// Put the cursor in the section, opening it first.
    pub(crate) fn focus(&self) {
        self.root.set_visible(self.status.borrow().is_some());
        if let Some(app) = self.app.upgrade() {
            app.set_fold(Fold::Git, false);
        }
        let first = self.files.borrow().first().map(|f| f.path.clone());
        match self.selected_path().or(first) {
            Some(path) => self.select_path(&path, true),
            None => {
                self.section.fold.grab_focus();
            }
        }
    }

    /// Fold the section away (the second press of the focus chord).
    pub(crate) fn fold(&self) {
        if let Some(app) = self.app.upgrade() {
            app.set_fold(Fold::Git, true);
        }
    }
}
