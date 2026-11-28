use adw::prelude::*;
use gtk::pango;

use crate::pane::set_class;

pub(crate) struct Row {
    pub root: gtk::Box,
    pub icon: gtk::Image,
    pub title: gtk::Label,
    pub detail: gtk::Label,
    pub trailing: gtk::Box,
}

impl Row {
    pub(crate) fn new() -> Self {
        let icon = gtk::Image::new();
        icon.set_visible(false);
        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.set_ellipsize(pango::EllipsizeMode::End);
        let detail = gtk::Label::new(None);
        detail.add_css_class("dim-label");
        detail.add_css_class("caption");
        detail.set_xalign(0.0);
        detail.set_hexpand(true);
        detail.set_ellipsize(pango::EllipsizeMode::End);
        let trailing = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        trailing.set_valign(gtk::Align::Center);
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.append(&icon);
        root.append(&title);
        root.append(&detail);
        root.append(&trailing);
        Self {
            root,
            icon,
            title,
            detail,
            trailing,
        }
    }

    pub(crate) fn of(root: &gtk::Widget) -> Option<Self> {
        let root = root.downcast_ref::<gtk::Box>()?.clone();
        let icon = root.first_child()?;
        let title = icon.next_sibling()?;
        let detail = title.next_sibling()?;
        let trailing = detail.next_sibling()?;
        Some(Self {
            icon: icon.downcast().ok()?,
            title: title.downcast().ok()?,
            detail: detail.downcast().ok()?,
            trailing: trailing.downcast().ok()?,
            root,
        })
    }

    pub(crate) fn set(&self, icon: Option<&str>, title: &str, detail: &str) {
        self.icon.set_visible(icon.is_some());
        if let Some(icon) = icon {
            self.icon.set_icon_name(Some(icon));
        }
        self.title.set_text(title);
        self.detail.set_text(detail);
    }

    pub(crate) fn stack(&self) {
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_hexpand(true);
        text.set_valign(gtk::Align::Center);
        self.root.insert_child_after(&text, Some(&self.icon));
        self.root.remove(&self.title);
        self.root.remove(&self.detail);
        text.append(&self.title);
        text.append(&self.detail);
    }

    pub(crate) fn set_open(&self, open: bool) {
        set_class(&self.title, "accent", open);
    }

    pub(crate) fn swap_on_hover(
        &self,
        status: &impl IsA<gtk::Widget>,
        hover: &impl IsA<gtk::Widget>,
    ) {
        hover.set_visible(false);
        self.trailing.append(status);
        self.trailing.append(hover);
        let motion = gtk::EventControllerMotion::new();
        let (status, hover) = (
            status.clone().upcast(),
            hover.clone().upcast::<gtk::Widget>(),
        );
        let swap = move |over: bool| {
            status.set_visible(!over);
            hover.set_visible(over);
        };
        let show = swap.clone();
        motion.connect_enter(move |_, _, _| {
            let show = show.clone();
            gtk::glib::idle_add_local_once(move || show(true));
        });
        motion.connect_leave(move |_| {
            let swap = swap.clone();
            gtk::glib::idle_add_local_once(move || swap(false));
        });
        self.root.add_controller(motion);
    }
}

pub(crate) fn struck(label: &gtk::Label, on: bool) {
    let attrs = pango::AttrList::new();
    if on {
        attrs.insert(pango::AttrInt::new_strikethrough(true));
    }
    label.set_attributes(Some(&attrs));
}
