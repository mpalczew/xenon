use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_core::WorkspaceId;
use xenon_git::status::{GitFile, GitStatus};

use crate::app::{App, defer};
use crate::git_rows::{make_row, more_row};
use crate::pane::focus_inside;
use crate::sidebar_ui::Row;

const MAX_ROWS: usize = 400;

pub(crate) struct GitView {
    pub root: gtk::Box,
    scroll: gtk::ScrolledWindow,
    clean: adw::StatusPage,
    branch: gtk::Label,
    list: gtk::ListBox,
    app: Weak<App>,
    ws: Cell<Option<WorkspaceId>>,
    status: RefCell<Option<GitStatus>>,
    shown: RefCell<Option<String>>,
    dir: RefCell<std::path::PathBuf>,
    files: RefCell<Vec<GitFile>>,
}

impl GitView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let branch = gtk::Label::new(None);
        branch.add_css_class("dim-label");
        branch.add_css_class("caption");
        branch.set_xalign(0.0);
        branch.set_margin_start(12);
        branch.set_margin_end(12);
        branch.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_vexpand(true);
        let clean = adw::StatusPage::new();
        clean.set_icon_name(Some("object-select-symbolic"));
        clean.set_title("Clean");
        clean.set_vexpand(true);
        clean.set_visible(false);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&branch);
        root.append(&scroll);
        root.append(&clean);
        let this = Rc::new(Self {
            root,
            scroll,
            clean,
            branch,
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
        self.list.connect_row_activated(move |_, row| {
            let index = row.index().max(0) as usize;
            defer(&weak, move |this| {
                this.open_row(index);
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
                defer(&this.app, move |app| app.focus_terminal());
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.root.add_controller(keys);
    }

    fn open_row(&self, index: usize) {
        let path = self.files.borrow().get(index).map(|f| f.path.clone());
        if let (Some(path), Some(app)) = (path, self.app.upgrade()) {
            app.open_diff(&path);
        }
    }

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
        if let Some(app) = self.app.upgrade() {
            app.sidebar.git_page.set_visible(status.is_some());
        }
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
        let is_clean = status.files.is_empty();
        self.clean.set_visible(is_clean);
        self.scroll.set_visible(!is_clean);
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

    pub(crate) fn has_focus(&self) -> bool {
        focus_inside(&self.root)
    }

    pub(crate) fn focus(&self) {
        let first = self.files.borrow().first().map(|f| f.path.clone());
        if let Some(path) = self.selected_path().or(first) {
            self.select_path(&path, true);
        }
    }
}
