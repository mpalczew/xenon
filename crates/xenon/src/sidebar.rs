use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use xenon_core::{WorkspaceDot, WorkspaceId};
use xenon_git::status::DirtState;
pub(crate) use xenon_store::SidebarPage;

use crate::app::{App, defer};
use crate::commands::Cmd;
use crate::files::FilesView;
use crate::git::GitHub;
use crate::git_section::GitView;
use crate::model::Model;
use crate::pane::set_class;
use crate::sidebar_ui::Row;

struct WsRow {
    id: WorkspaceId,
    row: gtk::ListBoxRow,
    title: gtk::Label,
    dot: gtk::Image,
    tick: gtk::Image,
}

pub(crate) struct Sidebar {
    pub root: adw::ToolbarView,
    pub header: adw::HeaderBar,
    pub files: Rc<FilesView>,
    pub git: Rc<GitView>,
    pub git_page: adw::ViewStackPage,
    stack: adw::ViewStack,
    pub list: gtk::ListBox,
    app: Weak<App>,
    rows: RefCell<Vec<WsRow>>,
    dragging: Rc<Cell<Option<usize>>>,
}

pub(crate) fn short_path(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

impl Sidebar {
    pub(crate) fn new(app: Weak<App>) -> Self {
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        let weak = app.clone();
        list.connect_row_activated(move |_, row| {
            let index = row.index().max(0) as usize;
            defer(&weak, move |app| {
                app.activate_workspace_index(index);
            });
        });
        let keys = gtk::EventControllerKey::new();
        let weak = app.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(app) = weak.upgrade()
            {
                glib::idle_add_local_once(move || app.focus_terminal());
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        list.add_controller(keys);
        list.add_css_class("xworkspaces");
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.set_valign(gtk::Align::Start);
        rule.set_can_target(false);
        rule.set_visible(false);
        rule.set_margin_start(6);
        rule.set_margin_end(6);
        for cap in [true, false] {
            if !cap {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                line.add_css_class("xdrop-line");
                line.set_hexpand(true);
                line.set_valign(gtk::Align::Center);
                rule.append(&line);
            }
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("xdrop-cap");
            rule.append(&dot);
        }
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&list));
        overlay.add_overlay(&rule);
        let dragging = Rc::new(Cell::new(None));
        Self::accept_drops(&list, &rule, &dragging, app.clone());

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&overlay));
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let files = FilesView::new(app.clone());
        let git = GitView::new(app.clone());
        let stack = adw::ViewStack::new();
        stack.set_vexpand(true);
        stack.add_titled_with_icon(
            &scroll,
            Some("workspaces"),
            "Workspaces",
            "view-grid-symbolic",
        );
        stack.add_titled_with_icon(&files.root, Some("files"), "Files", "folder-symbolic");
        let git_page =
            stack.add_titled_with_icon(&git.root, Some("git"), "Git", "document-edit-symbolic");
        let switcher = adw::InlineViewSwitcher::builder()
            .display_mode(adw::InlineViewSwitcherDisplayMode::Icons)
            .stack(&stack)
            .build();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&switcher));
        header.set_show_end_title_buttons(false);
        let root = adw::ToolbarView::new();
        root.add_css_class("sidebar-pane");
        root.add_top_bar(&header);
        root.set_content(Some(&stack));
        Self {
            root,
            header,
            files,
            git,
            git_page,
            stack,
            list,
            app,
            rows: RefCell::default(),
            dragging,
        }
    }

    pub(crate) fn show(&self, page: SidebarPage) {
        let (name, visible) = match page {
            SidebarPage::Workspaces => ("workspaces", true),
            SidebarPage::Files => ("files", true),
            SidebarPage::Git => ("git", self.git_page.is_visible()),
        };
        if visible {
            self.stack.set_visible_child_name(name);
        }
    }

    pub(crate) fn page(&self) -> SidebarPage {
        match self.stack.visible_child_name().as_deref() {
            Some("git") => SidebarPage::Git,
            Some("files") => SidebarPage::Files,
            _ => SidebarPage::Workspaces,
        }
    }

    fn accept_drops(
        list: &gtk::ListBox,
        rule: &gtk::Box,
        dragging: &Rc<Cell<Option<usize>>>,
        app: Weak<App>,
    ) {
        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let slot_at = |list: &gtk::ListBox, y: f64| {
            let rows = list.observe_children().n_items() as i32;
            let bounds = |i: i32| {
                let b = list.row_at_index(i)?.compute_bounds(list)?;
                Some((b.y() as f64, (b.y() + b.height()) as f64))
            };
            let slot = (0..rows)
                .find(|&i| bounds(i).is_some_and(|(top, bottom)| y < (top + bottom) / 2.0))
                .unwrap_or(rows);
            let line = match slot {
                0 => bounds(0).map(|b| b.0),
                s if s == rows => bounds(rows - 1).map(|b| b.1),
                s => bounds(s - 1).zip(bounds(s)).map(|(a, b)| (a.1 + b.0) / 2.0),
            };
            (slot as usize, line.unwrap_or(0.0))
        };
        let moves = {
            let dragging = dragging.clone();
            move |slot: usize| dragging.get().is_some_and(|d| slot != d && slot != d + 1)
        };
        target.connect_motion({
            let list = list.clone();
            let rule = rule.clone();
            let moves = moves.clone();
            move |_, _, y| {
                let (slot, line) = slot_at(&list, y);
                rule.set_margin_top(line as i32 - 3);
                rule.set_visible(moves(slot));
                gdk::DragAction::MOVE
            }
        });
        target.connect_leave({
            let rule = rule.clone();
            move |_| rule.set_visible(false)
        });
        target.connect_drop({
            let list = list.clone();
            let rule = rule.clone();
            move |_, value, _, y| {
                rule.set_visible(false);
                let (slot, _) = slot_at(&list, y);
                let Ok(from) = value.get::<u32>() else {
                    return false;
                };
                if !moves(slot) {
                    return false;
                }
                let end = slot == list.observe_children().n_items() as usize;
                defer(&app, move |app| {
                    app.reorder_workspace(from as usize, (!end).then_some(slot));
                });
                true
            }
        });
        list.add_controller(target);
    }

    pub(crate) fn rebuild(&self, model: &Model) {
        let mut rows = self.rows.borrow_mut();
        self.list.remove_all();
        rows.clear();
        for rec in &model.registry.workspaces {
            let line = Row::new();
            line.set(None, &rec.name, &short_path(&rec.display_root()));
            line.stack();
            line.root.set_tooltip_text(Some(&rec.display_root()));
            let dot = gtk::Image::from_icon_name("media-record-symbolic");
            let tick = gtk::Image::from_icon_name("document-edit-symbolic");
            let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            status.append(&dot);
            status.append(&tick);
            let close = gtk::Button::from_icon_name("window-close-symbolic");
            close.set_has_frame(false);
            close.set_focusable(false);
            close.set_tooltip_text(Some(&Cmd::CloseWorkspace.tip()));
            let weak = self.app.clone();
            let id = rec.id;
            close.connect_clicked(move |_| {
                defer(&weak, move |app| {
                    app.close_workspace_id(id);
                });
            });
            line.swap_on_hover(&status, &close);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&line.root));
            let drag = gtk::DragSource::new();
            drag.set_actions(gdk::DragAction::MOVE);
            drag.connect_prepare({
                let row = row.clone();
                move |_, _, _| {
                    Some(gdk::ContentProvider::for_value(
                        &(row.index().max(0) as u32).to_value(),
                    ))
                }
            });
            drag.connect_drag_begin({
                let row = row.clone();
                let dragging = self.dragging.clone();
                let name = rec.name.clone();
                move |_, drag| {
                    dragging.set(Some(row.index().max(0) as usize));
                    row.add_css_class("xdragging");
                    let ghost = gtk::Label::new(Some(&name));
                    ghost.add_css_class("xdrag-ghost");
                    ghost.set_margin_start(14);
                    ghost.set_margin_top(20);
                    if let Some(icon) =
                        gtk::DragIcon::for_drag(drag).downcast_ref::<gtk::DragIcon>()
                    {
                        // sync: answer or ordering before return: the icon window is new and belongs to this drag
                        icon.set_child(Some(&ghost));
                    }
                }
            });
            drag.connect_drag_end({
                let row = row.clone();
                let dragging = self.dragging.clone();
                move |_, _, _| {
                    dragging.set(None);
                    row.remove_css_class("xdragging");
                }
            });
            row.add_controller(drag);
            self.list.append(&row);
            rows.push(WsRow {
                id: rec.id,
                row,
                title: line.title.clone(),
                dot,
                tick,
            });
        }
    }

    pub(crate) fn paint(&self, model: &Model, git: &GitHub) {
        let active = model.active();
        for r in self.rows.borrow().iter() {
            if Some(r.id) == active {
                self.list.select_row(Some(&r.row));
            }
            set_class(&r.title, "heading", Some(r.id) == active);
            let dot = model.workspace_dot(r.id);
            let working = matches!(dot, Some(WorkspaceDot::Working));
            let attention = matches!(dot, Some(WorkspaceDot::Attention(_)));
            r.dot.set_visible(working || attention);
            set_class(&r.dot, "accent", working);
            set_class(&r.dot, "warning", attention);
            r.dot.set_opacity(1.0);
            let dirt = git.dirt(r.id);
            r.tick.set_visible(dirt != DirtState::Clean);
            set_class(&r.tick, "warning", dirt == DirtState::Dirty);
            set_class(&r.tick, "dim-label", dirt == DirtState::IndexOnly);
            r.tick.set_tooltip_text(match dirt {
                DirtState::Clean => None,
                DirtState::Dirty => Some("Changes since HEAD"),
                DirtState::IndexOnly => Some("Staged changes only"),
            });
        }
    }

    pub(crate) fn pulse(&self, opacity: f64) {
        for r in self.rows.borrow().iter() {
            if r.dot.has_css_class("accent") {
                r.dot.set_opacity(opacity);
            }
        }
    }
}

impl App {
    pub(crate) fn reveal_dir(self: &Rc<Self>, _ws: WorkspaceId, dir: &std::path::Path) {
        self.sidebar.root.set_visible(true);
        self.sidebar.files.reveal(dir);
    }

    pub(crate) fn toggle_sidebar(&self) {
        let shown = self.sidebar.root.is_visible();
        self.sidebar.root.set_visible(!shown);
    }

    pub(crate) fn focus_sidebar(&self) {
        self.sidebar.root.set_visible(true);
        match self.sidebar.page() {
            SidebarPage::Files => self.sidebar.files.focus(),
            SidebarPage::Git => self.sidebar.git.focus(),
            SidebarPage::Workspaces => {
                let model = self.model.borrow();
                let active = model.active();
                let index = model
                    .registry
                    .workspaces
                    .iter()
                    .position(|r| Some(r.id) == active);
                if let Some(row) = self.sidebar.list.row_at_index(index.unwrap_or(0) as i32) {
                    row.grab_focus();
                }
            }
        }
    }
}
