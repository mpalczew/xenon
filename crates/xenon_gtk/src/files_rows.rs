//! Rows of the Files tree: which entries show (`FilesModel`, plain data) and
//! the stock widget row drawn for each.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use adw::prelude::*;
use xenon_finder::tree::TreeEntry;

/// One visible row.
#[derive(Clone)]
pub(crate) struct Row {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) is_symlink: bool,
    pub(crate) expanded: bool,
    pub(crate) depth: usize,
}

/// Which folders are open and what each folder holds.
pub(crate) struct FilesModel {
    pub(crate) root: PathBuf,
    pub(crate) expanded: HashSet<PathBuf>,
    pub(crate) dirs: HashMap<PathBuf, Vec<TreeEntry>>,
}

impl FilesModel {
    pub(crate) fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        self.push(&self.root, 0, &mut out);
        out
    }

    fn push(&self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        for entry in self.dirs.get(dir).into_iter().flatten() {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            out.push(Row {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: entry.is_dir,
                is_symlink: entry.is_symlink,
                expanded,
                depth,
            });
            if expanded {
                self.push(&entry.path, depth + 1, out);
            }
        }
    }
}

fn file_icon(row: &Row) -> &'static str {
    if row.is_dir {
        return if row.expanded {
            "folder-open-symbolic"
        } else {
            "folder-symbolic"
        };
    }
    let ext = row
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
            "image-x-generic-symbolic"
        }
        "sh" | "bash" | "zsh" | "fish" => "utilities-terminal-symbolic",
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "7z" | "rar" => "package-x-generic-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

pub(crate) fn make_row(row: &Row) -> gtk::ListBoxRow {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    line.add_css_class("xfile");
    line.set_margin_start(6 + 14 * row.depth as i32);
    let chevron = gtk::Image::from_icon_name(if row.expanded {
        "pan-down-symbolic"
    } else {
        "pan-end-symbolic"
    });
    chevron.add_css_class("xchevron");
    chevron.set_opacity(if row.is_dir { 1.0 } else { 0.0 });
    let icon = gtk::Image::from_icon_name(file_icon(row));
    icon.add_css_class("xficon");
    let name = gtk::Label::new(Some(&row.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if row.is_symlink {
        name.add_css_class("xlink");
    }
    line.append(&chevron);
    line.append(&icon);
    line.append(&name);
    let widget = gtk::ListBoxRow::new();
    widget.set_child(Some(&line));
    widget.set_tooltip_text(Some(&row.path.to_string_lossy()));
    widget
}
