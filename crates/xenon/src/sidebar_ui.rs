//! The two widgets every sidebar section is made of: `Section` (header with a
//! title, an optional trailing widget and the fold control, then one list
//! body) and `Row` (icon, title, dim detail, trailing slot). Styling is the
//! stock libadwaita look; nothing here knows what a workspace or a file is.

use std::rc::Weak;

use adw::prelude::*;
use gtk::pango;

use crate::app::App;
use crate::model::Fold;
use crate::pane::set_class;

pub(crate) struct Section {
    pub root: gtk::Box,
    pub sep: gtk::Separator,
    pub fold: gtk::Button,
    head: gtk::Box,
    arrow: gtk::Image,
    body: gtk::Revealer,
}

impl Section {
    /// `cap` is the tallest the list grows before it scrolls; without one the
    /// list takes whatever height the section is given.
    pub(crate) fn new(
        title: &str,
        fold: Fold,
        list: &impl IsA<gtk::Widget>,
        cap: Option<i32>,
        app: &Weak<App>,
    ) -> Self {
        let label = gtk::Label::new(Some(title));
        label.add_css_class("caption-heading");
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let arrow = gtk::Image::from_icon_name("pan-down-symbolic");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        line.append(&arrow);
        line.append(&label);
        let button = gtk::Button::new();
        button.set_child(Some(&line));
        button.set_has_frame(false);
        button.set_hexpand(true);
        let weak = app.clone();
        button.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.toggle_fold(fold);
            }
        });
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        head.set_margin_start(6);
        head.set_margin_end(6);
        head.append(&button);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(list));
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_vexpand(cap.is_none());
        if let Some(cap) = cap {
            scroll.set_propagate_natural_height(true);
            scroll.set_max_content_height(cap);
        }
        let body = gtk::Revealer::new();
        body.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        body.set_transition_duration(160);
        body.set_reveal_child(true);
        body.set_child(Some(&scroll));

        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // Never grows on its own; the sidebar says which section does.
        root.set_vexpand(false);
        root.append(&sep);
        root.append(&head);
        root.append(&body);
        Self {
            root,
            sep,
            fold: button,
            head,
            arrow,
            body,
        }
    }

    /// A widget at the end of the header, beside the fold control.
    pub(crate) fn set_trailing(&self, widget: &impl IsA<gtk::Widget>) {
        self.head.append(widget);
    }

    /// Draw the fold the model decided.
    pub(crate) fn show_fold(&self, folded: bool) {
        self.body.set_reveal_child(!folded);
        self.arrow.set_icon_name(Some(if folded {
            "pan-end-symbolic"
        } else {
            "pan-down-symbolic"
        }));
    }

    /// A section with nothing to list is just its header: no fold arrow, no body.
    pub(crate) fn set_foldable(&self, foldable: bool) {
        self.arrow.set_visible(foldable);
        self.body.set_visible(foldable);
    }
}

/// One list row. The widgets are children of `root` in a fixed order, so a row
/// found in a list can be read back with `Row::of`.
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
        // Always shown, so its stretch pushes the trailing slot to the end.
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

    /// The row built by `Row::new` whose root is `root`.
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

    /// Put the detail under the title instead of beside it.
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

    /// Mark the row as the one shown in the main area.
    pub(crate) fn set_open(&self, open: bool) {
        set_class(&self.title, "accent", open);
    }

    /// While the pointer is over the row, `hover` replaces `status` in the
    /// trailing slot.
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
        motion.connect_enter(move |_, _, _| show(true));
        motion.connect_leave(move |_| swap(false));
        self.root.add_controller(motion);
    }
}

/// A strikethrough attribute list for a label.
pub(crate) fn struck(label: &gtk::Label, on: bool) {
    let attrs = pango::AttrList::new();
    if on {
        attrs.insert(pango::AttrInt::new_strikethrough(true));
    }
    label.set_attributes(Some(&attrs));
}
