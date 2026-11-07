//! The rows of the Git section: one per changed file, a Clean row, a note row.

use xenon_git::bar::{BarCell, change_bar};
use xenon_git::status::{ChangeKind, GitFile};

use gtk::prelude::*;

use crate::sidebar_ui::{Row, struck};

/// The stock color class for a kind of change.
fn kind_class(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Modified => "warning",
        ChangeKind::Added | ChangeKind::Untracked => "success",
        ChangeKind::Deleted | ChangeKind::Conflict => "error",
        ChangeKind::Renamed => "accent",
    }
}

pub(super) fn make_row(file: &GitFile, root: &std::path::Path) -> gtk::ListBoxRow {
    let line = Row::new();
    line.set(None, file.name(), &file.detail());
    line.title.add_css_class(kind_class(file.kind()));
    struck(&line.title, file.kind() == ChangeKind::Deleted);
    for (letter, kind) in file.badges() {
        let badge = gtk::Label::new(Some(&letter.to_string()));
        badge.add_css_class("caption-heading");
        badge.add_css_class(if letter == '?' {
            "dim-label"
        } else {
            kind_class(kind)
        });
        line.trailing.append(&badge);
    }
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    for cell in change_bar(file.added, file.removed) {
        let w = gtk::Image::from_icon_name("media-record-symbolic");
        w.set_pixel_size(6);
        w.add_css_class(match cell {
            BarCell::Added => "success",
            BarCell::Removed => "error",
            BarCell::Neutral => "dim-label",
        });
        bar.append(&w);
    }
    line.trailing.append(&bar);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&line.root));
    row.set_tooltip_text(Some(&file.full_tooltip(root)));
    row
}

/// A row that only says something: not selectable, not activatable.
fn note_row(row: &Row) -> gtk::ListBoxRow {
    let widget = gtk::ListBoxRow::new();
    widget.set_child(Some(&row.root));
    widget.set_activatable(false);
    widget.set_selectable(false);
    widget
}

pub(super) fn more_row(text: &str) -> gtk::ListBoxRow {
    let line = Row::new();
    line.set(None, "", text);
    note_row(&line)
}
