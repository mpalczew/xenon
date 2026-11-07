//! One pane: a tab strip of chips above a stack of tab bodies. Stock widgets
//! only (boxes, labels, a stack); CSS gives them their look.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::gdk;
use xenon_core::{PaneId, PaneLeaf, TabId, WorkspaceDot, WorkspaceId};

use crate::app::App;
use crate::model::Tab;

struct Chip {
    widget: gtk::Box,
    label: gtk::Label,
    rail: gtk::Box,
}

pub(crate) struct PaneView {
    pub root: gtk::Box,
    tabs: gtk::Box,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    chips: RefCell<HashMap<TabId, Chip>>,
}

impl PaneView {
    pub(crate) fn new(app: &Rc<App>, ws: WorkspaceId, pane: PaneId) -> Rc<Self> {
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let add = gtk::Button::from_icon_name("list-add-symbolic");
        add.add_css_class("flat");
        add.set_tooltip_text(Some("New terminal (Ctrl+N)"));
        add.set_focusable(false);
        let weak = Rc::downgrade(app);
        add.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.new_terminal_in(ws, pane);
            }
        });
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::External, gtk::PolicyType::Never);
        scroller.set_child(Some(&tabs));
        scroller.set_hexpand(true);
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        strip.add_css_class("xtabs");
        strip.append(&scroller);
        strip.append(&add);
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("xpane");
        root.append(&strip);
        root.append(&stack);
        Rc::new(Self {
            root,
            tabs,
            scroller,
            stack,
            chips: RefCell::default(),
        })
    }

    /// Make the pane show exactly `leaf`'s tabs. `bodies` holds each tab's
    /// widget; a body that sits in another pane's stack is moved here.
    pub(crate) fn sync(
        &self,
        app: &Rc<App>,
        ws: WorkspaceId,
        leaf: &PaneLeaf<Tab>,
        bodies: &HashMap<TabId, gtk::Widget>,
    ) {
        self.sync_chips(app, ws, leaf);
        let keep: Vec<&gtk::Widget> = leaf.tabs.iter().filter_map(|t| bodies.get(&t.id)).collect();
        let mut stale = Vec::new();
        let mut child = self.stack.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if !keep.contains(&&widget) {
                stale.push(widget);
            }
        }
        for widget in stale {
            self.stack.remove(&widget);
        }
        for tab in &leaf.tabs {
            let Some(body) = bodies.get(&tab.id) else {
                continue;
            };
            match body.parent() {
                Some(p) if p == *self.stack.upcast_ref::<gtk::Widget>() => {}
                Some(p) => {
                    if let Ok(other) = p.downcast::<gtk::Stack>() {
                        other.remove(body);
                    }
                    self.stack.add_child(body);
                }
                None => {
                    self.stack.add_child(body);
                }
            }
        }
        match leaf.active_tab().and_then(|t| bodies.get(&t.id)) {
            Some(body) => self.stack.set_visible_child(body),
            None => self.show_empty(),
        }
    }

    fn show_empty(&self) {
        if self.stack.child_by_name("empty").is_none() {
            let hint = gtk::Label::new(Some("Empty pane. Ctrl+N opens a terminal here."));
            hint.add_css_class("xempty-hint");
            self.stack.add_named(&hint, Some("empty"));
        }
        self.stack.set_visible_child_name("empty");
    }

    fn sync_chips(&self, app: &Rc<App>, ws: WorkspaceId, leaf: &PaneLeaf<Tab>) {
        let mut chips = self.chips.borrow_mut();
        while let Some(child) = self.tabs.first_child() {
            self.tabs.remove(&child);
        }
        chips.clear();
        for (index, tab) in leaf.tabs.iter().enumerate() {
            let chip = make_chip(app, ws, tab.id);
            chip.label.set_text(&tab.label(index));
            let selected = index == leaf.active;
            set_class(&chip.widget, "selected", selected);
            set_class(&chip.widget, "exited", tab.exited.is_some());
            self.tabs.append(&chip.widget);
            chips.insert(tab.id, chip);
        }
        let selected = chips.get(&leaf.tabs.get(leaf.active).map_or(TabId(0), |t| t.id));
        if let Some(chip) = selected {
            reveal(&self.scroller, &self.tabs, &chip.widget);
        }
    }

    /// Paint a chip's rail for `dot`. Selected tabs carry no status.
    pub(crate) fn paint(&self, tab: TabId, selected: bool, dot: Option<WorkspaceDot>) {
        let chips = self.chips.borrow();
        let Some(chip) = chips.get(&tab) else { return };
        let dot = if selected { None } else { dot };
        set_class(
            &chip.rail,
            "working",
            matches!(dot, Some(WorkspaceDot::Working)),
        );
        set_class(
            &chip.rail,
            "attention",
            matches!(dot, Some(WorkspaceDot::Attention(_))),
        );
        chip.rail.set_opacity(1.0);
        chip.widget.set_tooltip_text(dot.map(WorkspaceDot::tooltip));
    }

    /// Set the pulse opacity on the rails of the given working tabs.
    pub(crate) fn pulse(&self, working: &[TabId], opacity: f64) {
        let chips = self.chips.borrow();
        for tab in working {
            if let Some(chip) = chips.get(tab)
                && chip.rail.has_css_class("working")
            {
                chip.rail.set_opacity(opacity);
            }
        }
    }

    pub(crate) fn set_title(&self, tab: TabId, text: &str) {
        if let Some(chip) = self.chips.borrow().get(&tab) {
            chip.label.set_text(text);
        }
    }

    pub(crate) fn set_ring(&self, on: bool) {
        set_class(&self.root, "ring", on);
    }
}

pub(crate) fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

fn make_chip(app: &Rc<App>, ws: WorkspaceId, tab: TabId) -> Chip {
    let label = gtk::Label::new(None);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(24);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.add_css_class("xclose");
    close.set_focusable(false);
    close.set_valign(gtk::Align::Center);
    close.set_tooltip_text(Some("Close tab (Ctrl+W)"));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.set_margin_top(4);
    row.append(&label);
    row.append(&close);
    let rail = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    rail.add_css_class("xrail");
    let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
    widget.add_css_class("xchip");
    widget.append(&row);
    widget.append(&rail);

    let weak: Weak<App> = Rc::downgrade(app);
    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.connect_pressed(move |gesture, _, _, _| {
        let Some(app) = weak.upgrade() else { return };
        match gesture.current_button() {
            gdk::BUTTON_PRIMARY => app.select_tab_ui(ws, tab),
            gdk::BUTTON_MIDDLE => app.close_tab_ui(ws, tab),
            _ => {}
        }
    });
    widget.add_controller(click);
    let weak = Rc::downgrade(app);
    close.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            app.close_tab_ui(ws, tab);
        }
    });
    Chip {
        widget,
        label,
        rail,
    }
}

/// Scroll the strip so `chip` is fully visible.
fn reveal(scroller: &gtk::ScrolledWindow, tabs: &gtk::Box, chip: &gtk::Box) {
    let (scroller, tabs, chip) = (scroller.clone(), tabs.clone(), chip.clone());
    gtk::glib::idle_add_local_once(move || {
        let Some(bounds) = chip.compute_bounds(&tabs) else {
            return;
        };
        let adj = scroller.hadjustment();
        let (left, right) = (
            f64::from(bounds.x()),
            f64::from(bounds.x() + bounds.width()),
        );
        if left < adj.value() {
            adj.set_value(left);
        } else if right > adj.value() + adj.page_size() {
            adj.set_value(right - adj.page_size());
        }
    });
}
