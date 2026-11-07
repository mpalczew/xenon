//! The palette's result list: a store of `PaletteRow`s shown by a
//! `GtkListView` whose rows draw the title (matched letters in the accent
//! color), detail and badge.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::palette::PaletteRow;

/// `text` as Pango markup with the matched letters in the accent color.
pub(crate) fn markup(text: &str, hits: &[u32], accent: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for (i, c) in text.chars().enumerate() {
        let hit = hits.binary_search(&(i as u32)).is_ok();
        if hit && !open {
            out.push_str(&format!("<span foreground=\"{accent}\" weight=\"bold\">"));
            open = true;
        } else if !hit && open {
            out.push_str("</span>");
            open = false;
        }
        out.push_str(&glib::markup_escape_text(&c.to_string()));
    }
    if open {
        out.push_str("</span>");
    }
    out
}

/// The result list: a store of `PaletteRow`s shown by a `GtkListView` whose
/// rows draw the title (matched letters in the accent color), detail and badge.
pub(crate) fn row_list(
    accent: &Rc<RefCell<String>>,
) -> (gio::ListStore, gtk::SingleSelection, gtk::ListView) {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(true);
    selection.set_can_unselect(false);
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        title.add_css_class("xp-title");
        let detail = gtk::Label::new(None);
        detail.set_xalign(1.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        detail.add_css_class("xp-detail");
        let badge = gtk::Label::new(None);
        badge.add_css_class("xp-badge");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.add_css_class("xp-row");
        title.set_hexpand(true);
        row.append(&title);
        row.append(&detail);
        row.append(&badge);
        item.set_child(Some(&row));
    });
    let accent_for_bind = accent.clone();
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(obj), Some(row)) = (
            item.item().and_downcast::<glib::BoxedAnyObject>(),
            item.child().and_downcast::<gtk::Box>(),
        ) else {
            return;
        };
        let data = obj.borrow::<PaletteRow>();
        let mut kids = Vec::new();
        let mut child = row.first_child();
        while let Some(w) = child {
            child = w.next_sibling();
            kids.push(w);
        }
        if let [title, detail, badge] = &kids[..]
            && let (Some(title), Some(detail), Some(badge)) = (
                title.downcast_ref::<gtk::Label>(),
                detail.downcast_ref::<gtk::Label>(),
                badge.downcast_ref::<gtk::Label>(),
            )
        {
            title.set_markup(&markup(&data.title, &data.hits, &accent_for_bind.borrow()));
            detail.set_text(data.detail.as_deref().unwrap_or(""));
            detail.set_visible(data.detail.is_some());
            badge.set_text(data.badge.as_deref().unwrap_or(""));
            badge.set_visible(data.badge.is_some());
        }
        row.set_opacity(if data.enabled { 1.0 } else { 0.5 });
    });
    let view = gtk::ListView::new(Some(selection.clone()), Some(factory));
    view.add_css_class("xpalette-list");
    view.set_focusable(false);
    view.set_single_click_activate(false);

    (store, selection, view)
}
