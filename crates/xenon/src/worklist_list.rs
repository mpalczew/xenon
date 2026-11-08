//! The worklist list view: each section is a boxed list of items over
//! `.xenon/worklist.md`, with an "Add item…" row at its end. Parsing and every
//! edit live in `xenon_store::worklist`; this file draws and writes the result
//! back. Adding and editing an item use the capture form (`worklist.rs`); the
//! Markdown tab of the same file follows through its disk watching.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_store::worklist::entries::parse;
use xenon_store::worklist::view::inline_markup;
use xenon_store::worklist::{Target, capture_targets, read_source};

use crate::app::App;

mod write;

/// One focusable row: an item (its index in the parsed entries) or an "Add item…" row.
struct RowInfo {
    widget: gtk::Widget,
    target: Target,
    item: Option<usize>,
}

/// Where focus was, to put it back after the rows are redrawn.
enum Focus {
    Item(usize),
    Add(Target),
}

pub(crate) struct WorklistList {
    /// The tab header: title, add and menu buttons (the editor adds its view switch).
    pub(crate) header: gtk::Box,
    /// The list page itself.
    pub(crate) root: gtk::Box,
    actions: gtk::Box,
    workspace: PathBuf,
    app: Weak<App>,
    content: gtk::Box,
    error: gtk::Label,
    rows: RefCell<Vec<RowInfo>>,
    /// The source the rows were built from.
    source: RefCell<String>,
    hide_done: Cell<bool>,
}

fn menu_button(menu: &gio::Menu) -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_icon_name("view-more-symbolic");
    button.set_menu_model(Some(menu));
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button
}

fn item_with(label: &str, action: &str, target: Option<glib::Variant>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), target.as_ref());
    item
}

impl WorklistList {
    /// `workspace` is the folder that holds `.xenon`.
    pub(crate) fn new(workspace: PathBuf, app: Weak<App>) -> Rc<Self> {
        let title = gtk::Label::new(Some("Worklist"));
        title.add_css_class("heading");
        title.set_hexpand(true);
        let add = gtk::Button::from_icon_name("list-add-symbolic");
        add.set_tooltip_text(Some("Add Item"));
        add.set_action_name(Some("worklist.add"));
        add.set_focus_on_click(false);
        add.add_css_class("flat");
        let menu = gio::Menu::new();
        menu.append_item(&item_with(
            "Hide Completed",
            "worklist.hide-completed",
            None,
        ));
        let more = menu_button(&menu);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        actions.append(&add);
        actions.append(&more);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("toolbar");
        header.append(&title);
        header.append(&actions);

        let error = gtk::Label::new(None);
        error.set_wrap(true);
        error.set_xalign(0.0);
        error.add_css_class("error");
        error.set_margin_start(16);
        error.set_margin_end(16);
        error.set_visible(false);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_margin_top(8);
        content.set_margin_bottom(24);
        content.set_margin_start(16);
        content.set_margin_end(16);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(720);
        clamp.set_child(Some(&content));
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&clamp));
        scroller.set_vexpand(true);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&error);
        root.append(&scroller);

        let this = Rc::new(Self {
            header,
            root,
            actions,
            workspace,
            app,
            content,
            error,
            rows: RefCell::default(),
            source: RefCell::default(),
            hide_done: Cell::new(false),
        });
        this.install_actions();
        this
    }

    /// The add and menu buttons belong to the list, not to the Markdown text.
    pub(crate) fn show_actions(&self, on: bool) {
        self.actions.set_visible(on);
    }

    fn install_actions(self: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        let weak = Rc::downgrade(self);
        // An action that runs once the signal that raised it is over: most of
        // them redraw the rows that hold the widget that activated them.
        let add = |name: &str,
                   kind: Option<&glib::VariantTy>,
                   run: fn(&Rc<Self>, Option<glib::Variant>)| {
            let action = gio::SimpleAction::new(name, kind);
            let weak = weak.clone();
            action.connect_activate(move |_, parameter| {
                let (weak, parameter) = (weak.clone(), parameter.cloned());
                glib::idle_add_local_once(move || {
                    if let Some(this) = weak.upgrade() {
                        run(&this, parameter);
                    }
                });
            });
            group.add_action(&action);
        };
        let int = Some(glib::VariantTy::INT32);
        add("add", None, |this, _| this.add_via_form());
        add("new-section", None, |this, _| this.new_section());
        add("edit", int, |this, p| this.edit(index(&p)));
        add("delete", int, |this, p| this.delete(index(&p)));
        add("rename", int, |this, p| this.rename(index(&p)));
        add("delete-section", int, |this, p| {
            this.delete_section(index(&p))
        });
        add(
            "move",
            Some(glib::VariantTy::new("(ii)").unwrap()),
            |this, p| {
                let (item, target) = p.and_then(|p| p.get::<(i32, i32)>()).unwrap_or_default();
                this.move_to(item as usize, target as usize);
            },
        );
        let hide = gio::SimpleAction::new_stateful("hide-completed", None, &false.to_variant());
        let weak = Rc::downgrade(self);
        hide.connect_activate(move |action, _| {
            let on = !action
                .state()
                .and_then(|s| s.get::<bool>())
                .unwrap_or(false);
            action.set_state(&on.to_variant());
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.hide_done.set(on);
                    this.redraw();
                }
            });
        });
        group.add_action(&hide);
        self.root.insert_action_group("worklist", Some(&group));
        self.header.insert_action_group("worklist", Some(&group));
    }

    /// Show what is on disk now; used when the view is switched to.
    pub(crate) fn show(self: &Rc<Self>) {
        self.rebuild(read_source(&self.workspace), None);
    }

    /// The file may have changed on disk. Nothing happens while an "Add item…"
    /// row holds typed text (a redraw would lose it).
    pub(crate) fn refresh(self: &Rc<Self>) {
        let source = read_source(&self.workspace);
        if source != *self.source.borrow() && !self.typing() {
            self.rebuild(source, None);
        }
    }

    /// Put the cursor on the first row.
    pub(crate) fn focus(&self) {
        if let Some(first) = self.rows.borrow().first() {
            first.widget.grab_focus();
        }
    }

    fn typing(&self) -> bool {
        self.rows.borrow().iter().any(|row| {
            row.item.is_none()
                && row
                    .widget
                    .downcast_ref::<adw::EntryRow>()
                    .is_some_and(|entry| !entry.text().is_empty())
        })
    }

    fn redraw(self: &Rc<Self>) {
        let source = self.source.borrow().clone();
        self.rebuild(source, None);
    }

    fn set_error(&self, error: Option<&str>) {
        self.error.set_text(error.unwrap_or(""));
        self.error.set_visible(error.is_some());
    }

    /// The row that has focus now, as something to restore.
    fn current_focus(&self) -> Option<Focus> {
        let focus = self.root.root()?.focus()?;
        let rows = self.rows.borrow();
        let row = rows
            .iter()
            .find(|row| focus == row.widget || focus.is_ancestor(&row.widget))?;
        Some(match row.item {
            Some(index) => Focus::Item(index),
            None => Focus::Add(row.target.clone()),
        })
    }

    /// The section a new item goes to: where focus is, else the first.
    fn current_target(&self) -> Target {
        let focus = self.root.root().and_then(|root| root.focus());
        let rows = self.rows.borrow();
        let found = focus.and_then(|focus| {
            rows.iter()
                .find(|row| focus == row.widget || focus.is_ancestor(&row.widget))
                .map(|row| row.target.clone())
        });
        found.unwrap_or_else(|| rows.first().map(|r| r.target.clone()).unwrap_or_default())
    }

    // ---- drawing ----

    fn rebuild(self: &Rc<Self>, source: String, focus: Option<Focus>) {
        let focus = focus.or_else(|| self.current_focus());
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        let parsed = parse(&source);
        let targets = capture_targets(&source);
        let mut rows = Vec::new();
        for target in &targets {
            let region = match target {
                Target::Top => None,
                Target::Section { index, .. } => Some(*index),
            };
            let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
            group.set_margin_top(20);
            if let Target::Section { index, title } = target {
                group.append(&self.section_header(*index, title));
            }
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            for (i, entry) in parsed.entries.iter().enumerate() {
                if entry.section != region || (self.hide_done.get() && entry.checked == Some(true))
                {
                    continue;
                }
                let row = self.item_row(i, entry, &targets);
                list.append(&row);
                rows.push(RowInfo {
                    widget: row.upcast(),
                    target: target.clone(),
                    item: Some(i),
                });
            }
            let add = self.add_row(target);
            list.append(&add);
            rows.push(RowInfo {
                widget: add.upcast(),
                target: target.clone(),
                item: None,
            });
            group.append(&list);
            self.content.append(&group);
        }
        let new = gtk::Button::with_label("New Section");
        new.set_action_name(Some("worklist.new-section"));
        new.set_halign(gtk::Align::Start);
        new.set_margin_top(20);
        self.content.append(&new);

        // Put focus back: the same item (or the one now in its place), or the
        // same section's "Add item…" row.
        let restore = focus.and_then(|focus| match focus {
            Focus::Add(target) => rows.iter().find(|r| r.item.is_none() && r.target == target),
            Focus::Item(at) => rows
                .iter()
                .find(|r| r.item.is_some_and(|i| i >= at))
                .or_else(|| rows.iter().rev().find(|r| r.item.is_some())),
        });
        if let Some(row) = restore {
            row.widget.grab_focus();
        }
        *self.rows.borrow_mut() = rows;
        *self.source.borrow_mut() = source;
    }

    fn section_header(&self, index: usize, title: &str) -> gtk::Box {
        let name = gtk::Label::new(Some(if title.is_empty() { "Untitled" } else { title }));
        name.add_css_class("heading");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let menu = gio::Menu::new();
        let at = Some((index as i32).to_variant());
        menu.append_item(&item_with("Rename", "worklist.rename", at.clone()));
        menu.append_item(&item_with("Delete", "worklist.delete-section", at));
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&name);
        header.append(&menu_button(&menu));
        header
    }

    fn item_row(
        self: &Rc<Self>,
        index: usize,
        entry: &xenon_store::worklist::entries::Entry,
        targets: &[Target],
    ) -> adw::ActionRow {
        let done = entry.checked == Some(true);
        let row = adw::ActionRow::new();
        let title = inline_markup(&entry.title);
        row.set_title(&if done {
            format!("<s>{title}</s>")
        } else {
            title
        });
        let bullets: Vec<String> = entry
            .details
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(inline_markup)
            .collect();
        if !entry.editable {
            row.set_subtitle("Edit this one in Markdown");
        } else if !bullets.is_empty() {
            row.set_subtitle(&bullets.join(" \u{b7} "));
        }
        if done {
            row.add_css_class("dim-label");
        }
        let at = Some((index as i32).to_variant());
        if let Some(checked) = entry.checked {
            let check = gtk::CheckButton::new();
            check.add_css_class("selection-mode");
            check.set_valign(gtk::Align::Center);
            check.set_active(checked);
            check.set_sensitive(entry.editable);
            check.set_tooltip_text(Some("Mark as done"));
            let weak = Rc::downgrade(self);
            check.connect_toggled(move |_| {
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(this) = weak.upgrade() {
                        this.toggle(index);
                    }
                });
            });
            row.add_prefix(&check);
        }
        if entry.editable {
            let menu = gio::Menu::new();
            menu.append_item(&item_with("Edit", "worklist.edit", at.clone()));
            if targets.len() > 1 {
                let moves = gio::Menu::new();
                for (t, target) in targets.iter().enumerate() {
                    if entry.section == target_index(target) {
                        continue;
                    }
                    let to = Some((index as i32, t as i32).to_variant());
                    moves.append_item(&item_with(target.label(), "worklist.move", to));
                }
                menu.append_submenu(Some("Move to Section"), &moves);
            }
            menu.append_item(&item_with("Delete", "worklist.delete", at.clone()));
            row.add_suffix(&menu_button(&menu));
            row.set_activatable(true);
            row.set_action_name(Some("worklist.edit"));
            row.set_action_target_value(at.as_ref());
        }
        row
    }

    fn add_row(self: &Rc<Self>, target: &Target) -> adw::EntryRow {
        let row = adw::EntryRow::new();
        row.set_title("Add item\u{2026}");
        row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
        let weak = Rc::downgrade(self);
        let target = target.clone();
        row.connect_entry_activated(move |row| {
            let (weak, target, text) = (weak.clone(), target.clone(), row.text().to_string());
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.add(&target, &text);
                }
            });
        });
        row
    }
}

fn index(parameter: &Option<glib::Variant>) -> usize {
    parameter
        .as_ref()
        .and_then(|p| p.get::<i32>())
        .unwrap_or_default() as usize
}

fn target_index(target: &Target) -> Option<usize> {
    match target {
        Target::Top => None,
        Target::Section { index, .. } => Some(*index),
    }
}
