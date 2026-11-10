use adw::prelude::*;
use gtk::{gio, glib};

use crate::palette::PaletteRow;

pub(crate) fn markup(text: &str, hits: &[u32]) -> String {
    let mut out = String::new();
    let mut open = false;
    for (i, c) in text.chars().enumerate() {
        let hit = hits.binary_search(&(i as u32)).is_ok();
        if hit != open {
            out.push_str(if hit { "<b>" } else { "</b>" });
            open = hit;
        }
        out.push_str(&glib::markup_escape_text(&c.to_string()));
    }
    if open {
        out.push_str("</b>");
    }
    out
}

fn label(classes: &[&str], xalign: f32) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(xalign);
    label.set_css_classes(classes);
    label
}

pub(crate) fn row_list() -> (gio::ListStore, gtk::SingleSelection, gtk::ListView) {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_can_unselect(false);
    let factory = gtk::SignalListItemFactory::new();
    // sync: setup fills a new row widget that no list holds yet.
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let title = label(&[], 0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        title.set_hexpand(true);
        let detail = label(&["caption", "dim-label"], 1.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        let badge = label(&["caption-heading", "dim-label"], 0.5);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.append(&title);
        row.append(&detail);
        row.append(&badge);
        item.set_child(Some(&row));
    });
    // sync: a list factory must fill the row it is binding right now.
    factory.connect_bind(|_, item| {
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
        let title = row.first_child().and_downcast::<gtk::Label>();
        let detail = title.as_ref().and_then(|w| w.next_sibling());
        let badge = detail.as_ref().and_then(|w| w.next_sibling());
        if let (Some(title), Some(detail), Some(badge)) = (
            title,
            detail.and_downcast::<gtk::Label>(),
            badge.and_downcast::<gtk::Label>(),
        ) {
            title.set_markup(&markup(&data.title, &data.hits));
            detail.set_text(data.detail.as_deref().unwrap_or(""));
            detail.set_visible(data.detail.is_some());
            badge.set_text(data.badge.as_deref().unwrap_or(""));
            badge.set_visible(data.badge.is_some());
        }
        row.set_sensitive(data.enabled);
    });
    let view = gtk::ListView::new(Some(selection.clone()), Some(factory));
    view.add_css_class("navigation-sidebar");
    view.set_focusable(false);
    view.set_single_click_activate(true);
    (store, selection, view)
}

/// Close `dialog` like Escape when a click lands outside its `content`.
/// libadwaita has no property for this.
pub(crate) fn close_on_outside_click(dialog: &adw::Dialog, content: &impl IsA<gtk::Widget>) {
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (dialog_weak, content) = (dialog.downgrade(), content.clone().upcast::<gtk::Widget>());
    click.connect_pressed(move |_, _, x, y| {
        let Some(dialog) = dialog_weak.upgrade() else {
            return;
        };
        let inside = content
            .compute_bounds(&dialog)
            .is_some_and(|b| b.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)));
        if !inside {
            // sync: AdwDialog.close animates out; nothing is destroyed during this event.
            dialog.close();
        }
    });
    dialog.add_controller(click);
}
