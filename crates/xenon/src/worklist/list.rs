use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gio, glib};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use xenon_worklist::{Group, Item, Place, Snapshot, read};

use crate::app::{App, defer};

struct RowInfo {
    widget: gtk::Widget,
    place: Place,
    item: Option<Item>,
}

enum Focus {
    Row(usize),
    Add(Place),
}

pub(crate) struct WorklistList {
    pub(crate) header: gtk::Box,
    pub(crate) root: gtk::Box,
    actions: gtk::Box,
    pub(super) workspace: PathBuf,
    pub(super) app: Weak<App>,
    content: gtk::Box,
    error: gtk::Label,
    rows: RefCell<Vec<RowInfo>>,
    pub(super) snapshot: RefCell<Snapshot>,
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

fn inline_markup(line: &str) -> String {
    let mut events = Parser::new_ext(line, Options::ENABLE_STRIKETHROUGH);
    if !matches!(events.next(), Some(Event::Start(Tag::Paragraph))) {
        return glib::markup_escape_text(line).into();
    }
    let mut out = String::new();
    for event in events {
        match event {
            Event::Start(Tag::Strong) => out.push_str("<b>"),
            Event::End(TagEnd::Strong) => out.push_str("</b>"),
            Event::Start(Tag::Emphasis) => out.push_str("<i>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</i>"),
            Event::Start(Tag::Strikethrough) => out.push_str("<s>"),
            Event::End(TagEnd::Strikethrough) => out.push_str("</s>"),
            Event::Start(Tag::Link { .. }) => out.push_str("<u>"),
            Event::End(TagEnd::Link) => out.push_str("</u>"),
            Event::Text(piece) | Event::Html(piece) | Event::InlineHtml(piece) => {
                out.push_str(&glib::markup_escape_text(&piece));
            }
            Event::Code(piece) => {
                out.push_str("<tt>");
                out.push_str(&glib::markup_escape_text(&piece));
                out.push_str("</tt>");
            }
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out
}

impl WorklistList {
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
            snapshot: RefCell::default(),
            hide_done: Cell::new(false),
        });
        this.install_actions();
        this
    }

    pub(crate) fn show_actions(&self, on: bool) {
        self.actions.set_visible(on);
    }

    fn install_actions(self: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        let weak = Rc::downgrade(self);
        let add = |name: &str,
                   kind: Option<&glib::VariantTy>,
                   run: fn(&Rc<Self>, Option<glib::Variant>)| {
            let action = gio::SimpleAction::new(name, kind);
            let weak = weak.clone();
            action.connect_activate(move |_, parameter| {
                let (weak, parameter) = (weak.clone(), parameter.cloned());
                defer(&weak, move |this| {
                    run(&this, parameter);
                });
            });
            group.add_action(&action);
        };
        let int = Some(glib::VariantTy::INT32);
        add("add", None, |this, _| this.add_via_form());
        add("new-section", None, |this, _| this.new_section());
        // The i32 is a row (items) or group (sections) index in the snapshot rendered last.
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
                let (row, group) = p.and_then(|p| p.get::<(i32, i32)>()).unwrap_or_default();
                this.move_to(row as usize, group as usize);
            },
        );
        let hide = gio::SimpleAction::new_stateful("hide-completed", None, &false.to_variant());
        let weak = Rc::downgrade(self);
        // sync: answer or ordering before return: a stateful action sets its own state in its activate; the list change is deferred.
        hide.connect_activate(move |action, _| {
            let on = !action
                .state()
                .and_then(|s| s.get::<bool>())
                .unwrap_or(false);
            action.set_state(&on.to_variant());
            defer(&weak, move |this| {
                this.hide_done.set(on);
                this.redraw();
            });
        });
        group.add_action(&hide);
        self.root.insert_action_group("worklist", Some(&group));
        self.header.insert_action_group("worklist", Some(&group));
    }

    pub(crate) fn show(self: &Rc<Self>) {
        self.rebuild(read(&self.workspace).unwrap_or_default());
    }

    pub(crate) fn refresh(self: &Rc<Self>) {
        let now = read(&self.workspace).unwrap_or_default();
        if now != *self.snapshot.borrow() && !self.typing() {
            self.rebuild(now);
        }
    }

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

    pub(super) fn redraw(self: &Rc<Self>) {
        let snapshot = self.snapshot.borrow().clone();
        self.rebuild(snapshot);
    }

    pub(super) fn set_error(&self, error: Option<&str>) {
        self.error.set_text(error.unwrap_or(""));
        self.error.set_visible(error.is_some());
    }

    fn current_focus(&self) -> Option<Focus> {
        let focus = self.root.root()?.focus()?;
        let rows = self.rows.borrow();
        let at = rows
            .iter()
            .position(|row| focus == row.widget || focus.is_ancestor(&row.widget))?;
        Some(match rows[at].item {
            Some(_) => Focus::Row(at),
            None => Focus::Add(rows[at].place.clone()),
        })
    }

    pub(super) fn current_place(&self) -> Place {
        let focus = self.root.root().and_then(|root| root.focus());
        let rows = self.rows.borrow();
        let found = focus.and_then(|focus| {
            rows.iter()
                .find(|row| focus == row.widget || focus.is_ancestor(&row.widget))
                .map(|row| row.place.clone())
        });
        found.unwrap_or_else(|| rows.first().map(|r| r.place.clone()).unwrap_or_default())
    }

    pub(super) fn row_item(&self, row: usize) -> Option<Item> {
        self.rows.borrow().get(row)?.item.clone()
    }

    pub(super) fn rebuild(self: &Rc<Self>, snapshot: Snapshot) {
        let focus = self.current_focus();
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        let mut rows = Vec::new();
        for (g, group) in snapshot.groups.iter().enumerate() {
            let region = gtk::Box::new(gtk::Orientation::Vertical, 8);
            region.set_margin_top(20);
            if group.place != Place::Top {
                region.append(&self.section_header(g, &group.label));
            }
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            for item in &group.items {
                if self.hide_done.get() && item.done == Some(true) {
                    continue;
                }
                let row = self.item_row(rows.len(), item, (g, &snapshot.groups));
                list.append(&row);
                rows.push(RowInfo {
                    widget: row.upcast(),
                    place: group.place.clone(),
                    item: Some(item.clone()),
                });
            }
            let add = self.add_row(&group.place);
            list.append(&add);
            rows.push(RowInfo {
                widget: add.upcast(),
                place: group.place.clone(),
                item: None,
            });
            region.append(&list);
            self.content.append(&region);
        }
        let new = gtk::Button::with_label("New Section");
        new.set_action_name(Some("worklist.new-section"));
        new.set_halign(gtk::Align::Start);
        new.set_margin_top(20);
        self.content.append(&new);

        let restore = focus.and_then(|focus| match focus {
            Focus::Add(place) => rows.iter().find(|r| r.item.is_none() && r.place == place),
            Focus::Row(at) => rows.get(at).or(rows.last()),
        });
        if let Some(row) = restore {
            row.widget.grab_focus();
        }
        *self.rows.borrow_mut() = rows;
        *self.snapshot.borrow_mut() = snapshot;
    }

    fn section_header(&self, group: usize, title: &str) -> gtk::Box {
        let name = gtk::Label::new(Some(if title.is_empty() { "Untitled" } else { title }));
        name.add_css_class("heading");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let menu = gio::Menu::new();
        let at = Some((group as i32).to_variant());
        menu.append_item(&item_with("Rename", "worklist.rename", at.clone()));
        menu.append_item(&item_with("Delete", "worklist.delete-section", at));
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.append(&name);
        header.append(&menu_button(&menu));
        header
    }

    fn item_row(
        self: &Rc<Self>,
        row_index: usize,
        item: &Item,
        (group, groups): (usize, &[Group]),
    ) -> adw::ActionRow {
        let done = item.done == Some(true);
        let row = adw::ActionRow::new();
        let title = inline_markup(&item.title);
        row.set_title(&if done {
            format!("<s>{title}</s>")
        } else {
            title
        });
        let bullets: Vec<String> = item
            .details
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(inline_markup)
            .collect();
        if !item.editable {
            row.set_subtitle("Edit this one in Markdown");
        } else if !bullets.is_empty() {
            row.set_subtitle(&bullets.join(" \u{b7} "));
        }
        if done {
            row.add_css_class("dim-label");
        }
        let at = Some((row_index as i32).to_variant());
        if let Some(checked) = item.done {
            let check = gtk::CheckButton::new();
            check.add_css_class("selection-mode");
            check.set_valign(gtk::Align::Center);
            check.set_active(checked);
            check.set_sensitive(item.editable);
            check.set_tooltip_text(Some("Mark as done"));
            let weak = Rc::downgrade(self);
            let key = item.key.clone();
            check.connect_toggled(move |check| {
                let (key, done) = (key.clone(), check.is_active());
                defer(&weak, move |this| {
                    this.toggle(key, done);
                });
            });
            row.add_prefix(&check);
        }
        if item.editable {
            let menu = gio::Menu::new();
            menu.append_item(&item_with("Edit", "worklist.edit", at.clone()));
            if groups.len() > 1 {
                let moves = gio::Menu::new();
                for (g, other) in groups.iter().enumerate().filter(|(g, _)| *g != group) {
                    let to = Some((row_index as i32, g as i32).to_variant());
                    moves.append_item(&item_with(&other.label, "worklist.move", to));
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

    fn add_row(self: &Rc<Self>, place: &Place) -> adw::EntryRow {
        let row = adw::EntryRow::new();
        row.set_title("Add item\u{2026}");
        row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
        let weak = Rc::downgrade(self);
        let place = place.clone();
        row.connect_entry_activated(move |row| {
            let (weak, place, text) = (weak.clone(), place.clone(), row.text().to_string());
            defer(&weak, move |this| {
                this.add(&place, &text);
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
