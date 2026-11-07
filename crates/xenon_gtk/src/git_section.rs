//! The Git section of the sidebar: branch and count in the header, one row per
//! changed file (status badges, colored name, muted folder, five-cell change
//! bar), or a Clean row. Rows come from `xenon_git`'s status; this
//! file only makes widgets and moves the keyboard.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use serde_json::json;
use xenon_core::WorkspaceId;
use xenon_git::status::{GitFile, GitStatus};

use crate::app::App;
use crate::git_rows::{clean_row, make_row, more_row};
use crate::model::Fold;
use crate::pane::focus_inside;
use crate::sidebar_ui::{Row, Section};

/// Rows drawn at most; a repo with more changes shows the first ones and a
/// note, so a huge dirty tree cannot make the sidebar slow.
const MAX_ROWS: usize = 400;

pub(crate) struct GitView {
    pub root: gtk::Box,
    section: Section,
    branch: gtk::Label,
    count: gtk::Label,
    list: gtk::ListBox,
    app: Weak<App>,
    ws: Cell<Option<WorkspaceId>>,
    status: RefCell<Option<GitStatus>>,
    shown: RefCell<Option<String>>,
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
        let trailing = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        trailing.append(&branch);
        trailing.append(&count);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        let section = Section::new("GIT", Fold::Git, &list, Some(230), &app);
        section.set_trailing(&trailing);
        let root = section.root.clone();
        root.set_visible(false);
        let this = Rc::new(Self {
            root,
            section,
            branch,
            count,
            list,
            app,
            ws: Cell::new(None),
            status: RefCell::new(None),
            shown: RefCell::new(None),
            files: RefCell::default(),
        });
        this.wire();
        this
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.list.connect_row_activated(move |_, row| {
            if let Some(this) = weak.upgrade() {
                this.open_row(row.index().max(0) as usize);
            }
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
                    app.focus_terminal();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.root.add_controller(keys);
        // Scripts read `git-state`: keep it current when focus or selection moves.
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_enter(move |_| {
            if let Some(this) = weak.upgrade() {
                this.publish();
            }
        });
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            if let Some(this) = weak.upgrade() {
                this.publish();
            }
        });
        self.root.add_controller(focus);
        let weak = Rc::downgrade(self);
        self.list.connect_row_selected(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.publish();
            }
        });
    }

    fn publish(&self) {
        if let Some(app) = self.app.upgrade() {
            app.publish_git();
        }
    }

    /// Draw the fold the model decided.
    pub(crate) fn show_fold(&self, collapsed: bool) {
        self.section.show_fold(collapsed);
        self.publish();
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
    ) {
        let same = self.ws.get() == ws
            && self.status.borrow().as_ref() == status
            && *self.shown.borrow() == shown;
        if same {
            return;
        }
        let had_focus = focus_inside(&self.root);
        let keep = self.selected_path();
        let rebuild = self.ws.get() != ws || self.status.borrow().as_ref() != status;
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
        self.publish();
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
        if status.files.is_empty() {
            self.list.append(&clean_row(&status.branch));
            return;
        }
        let shown: Vec<GitFile> = status.files.iter().take(MAX_ROWS).cloned().collect();
        for file in &shown {
            self.list.append(&make_row(file));
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
        self.publish();
    }

    /// Fold the section away (the second press of the focus chord).
    pub(crate) fn fold(&self) {
        if let Some(app) = self.app.upgrade() {
            app.set_fold(Fold::Git, true);
        }
    }

    /// Move the selection by `delta` rows (scripts).
    pub(crate) fn key(&self, key: &str) {
        let n = self.files.borrow().len() as i32;
        let at = self.list.selected_row().map_or(-1, |r| r.index());
        match key {
            "down" | "up" if n > 0 => {
                let next = (at + if key == "down" { 1 } else { -1 }).clamp(0, n - 1);
                if let Some(row) = self.list.row_at_index(next) {
                    self.list.select_row(Some(&row));
                    row.grab_focus();
                }
                self.publish();
            }
            "enter" if at >= 0 => self.open_row(at as usize),
            "escape" => {
                if let Some(app) = self.app.upgrade() {
                    app.focus_terminal();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn snapshot(&self) -> serde_json::Value {
        let status = self.status.borrow();
        let rows: Vec<_> = self
            .files
            .borrow()
            .iter()
            .map(|f| {
                json!({
                    "path": f.path,
                    "badges": f.badges().iter().map(|(c, _)| c.to_string()).collect::<String>(),
                    "added": f.added,
                    "removed": f.removed,
                })
            })
            .collect();
        json!({
            "visible": self.root.is_visible(),
            "branch": status.as_ref().map(|s| s.branch.clone()),
            "count": status.as_ref().map_or(0, |s| s.files.len()),
            "collapsed": self.section.folded(),
            "focused": self.has_focus(),
            "selected": self.selected_path(),
            "shown": *self.shown.borrow(),
            "rows": rows,
        })
    }
}
