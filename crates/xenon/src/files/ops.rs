//! Files section, part three: managing files. The context menu (right click,
//! Menu key, Shift+F10), name prompts, trash, and drag and drop to move or copy.
//! What a name may be and where a drop lands is decided in
//! `xenon_finder::fileops`; this file only asks and shows the answer.
//!
//! Every action starts from a GTK signal, so each one captures plain paths and
//! does its work in an idle callback or a future, never inside the signal.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_finder::fileops;

use super::{FilesView, path_of, row_path};

/// What the open menu is about: a file or folder, or the empty area (`None`).
#[derive(Clone)]
pub(super) struct Target {
    pub(super) path: PathBuf,
    pub(super) is_dir: bool,
}

const ACTIONS: [&str; 8] = [
    "new-file",
    "new-folder",
    "rename",
    "trash",
    "copy-path",
    "copy-relative-path",
    "open",
    "show",
];

fn menu_model(on_item: bool) -> gio::Menu {
    let item = |label: &str, action: &str, accel: Option<&str>| {
        let item = gio::MenuItem::new(Some(label), Some(&format!("files.{action}")));
        if let Some(accel) = accel {
            item.set_attribute_value("accel", Some(&accel.to_variant()));
        }
        item
    };
    let menu = gio::Menu::new();
    let make = gio::Menu::new();
    make.append_item(&item("New File\u{2026}", "new-file", None));
    make.append_item(&item("New Folder\u{2026}", "new-folder", None));
    menu.append_section(None, &make);
    if on_item {
        let change = gio::Menu::new();
        change.append_item(&item("Rename\u{2026}", "rename", Some("F2")));
        change.append_item(&item("Move to Trash", "trash", Some("Delete")));
        menu.append_section(None, &change);
        let copy = gio::Menu::new();
        copy.append_item(&item("Copy Path", "copy-path", None));
        copy.append_item(&item("Copy Relative Path", "copy-relative-path", None));
        menu.append_section(None, &copy);
        let open = gio::Menu::new();
        open.append_item(&item("Open in Default App", "open", None));
        open.append_item(&item("Show in File Manager", "show", None));
        menu.append_section(None, &open);
    }
    menu
}

/// The row a widget in the list belongs to, as a plain path.
pub(super) fn target_of(expander: &gtk::TreeExpander) -> Option<Target> {
    let item = expander.list_row()?.item()?;
    Some(Target {
        path: path_of(&item)?,
        is_dir: super::is_dir(&item),
    })
}

impl FilesView {
    pub(super) fn wire_ops(self: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        for name in ACTIONS {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.run_action(name);
                }
            });
            group.add_action(&action);
        }
        self.list.insert_action_group("files", Some(&group));
        self.menu.set_parent(&self.list);
        self.menu.set_has_arrow(false);
        self.menu.set_halign(gtk::Align::Start);
        let menu = self.menu.clone();
        // sync: the popover must leave its parent while the parent is destroyed.
        self.list.connect_destroy(move |_| menu.unparent());

        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |_, _, x, y| {
            let Some(this) = weak.upgrade() else { return };
            let target = this
                .list
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .and_then(|w| w.ancestor(gtk::TreeExpander::static_type()))
                .and_downcast::<gtk::TreeExpander>()
                .and_then(|e| target_of(&e));
            let at = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            let weak = Rc::downgrade(&this);
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.popup(target, at);
                }
            });
        });
        self.list.add_controller(click);
        self.wire_drop();
    }

    /// Show the menu for `target` (`None`: the empty area) at `at`.
    fn popup(self: &Rc<Self>, target: Option<Target>, at: gdk::Rectangle) {
        if let Some(target) = &target {
            self.select_path(&target.path, false);
        }
        self.menu
            .set_menu_model(Some(&menu_model(target.is_some())));
        *self.ctx.borrow_mut() = target;
        self.menu.set_pointing_to(Some(&at));
        self.menu.popup();
    }

    /// Menu key or Shift+F10: the menu for the selected row.
    pub(super) fn popup_selected(self: &Rc<Self>) -> bool {
        let Some(row) = self.selected_row() else {
            return false;
        };
        let (Some(path), Some(item)) = (row_path(&row), row.item()) else {
            return false;
        };
        let target = Target {
            path,
            is_dir: super::is_dir(&item),
        };
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = weak.upgrade() else { return };
            let at = this.row_rect(&target.path);
            this.popup(Some(target), at);
        });
        true
    }

    fn row_rect(&self, path: &Path) -> gdk::Rectangle {
        let found = self.bound.borrow().iter().find_map(|item| {
            let expander = item.child().and_downcast::<gtk::TreeExpander>()?;
            let target = target_of(&expander)?;
            let bounds = (target.path == path)
                .then(|| expander.compute_bounds(&self.list))
                .flatten()?;
            Some(gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            ))
        });
        found.unwrap_or_else(|| gdk::Rectangle::new(0, 0, 1, 1))
    }

    /// F2 and Delete act on the selected row.
    pub(super) fn act_on_selected(self: &Rc<Self>, action: &'static str) -> bool {
        let Some(row) = self.selected_row() else {
            return false;
        };
        let (Some(path), Some(item)) = (row_path(&row), row.item()) else {
            return false;
        };
        let target = Target {
            path,
            is_dir: super::is_dir(&item),
        };
        self.defer_action(action, Some(target));
        true
    }

    fn run_action(self: &Rc<Self>, action: &'static str) {
        let target = self.ctx.borrow().clone();
        self.defer_action(action, target);
    }

    fn defer_action(self: &Rc<Self>, action: &'static str, target: Option<Target>) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(this) = weak.upgrade() {
                this.do_action(action, target);
            }
        });
    }

    fn do_action(self: &Rc<Self>, action: &str, target: Option<Target>) {
        let Some(root) = self.root_dir.borrow().clone() else {
            return;
        };
        let dir = match &target {
            Some(t) => fileops::new_item_dir(&t.path, t.is_dir),
            None => root.clone(),
        };
        let Some(Target { path, .. }) = target else {
            match action {
                "new-file" => self.new_item(dir, false),
                "new-folder" => self.new_item(dir, true),
                _ => {}
            }
            return;
        };
        match action {
            "new-file" => self.new_item(dir, false),
            "new-folder" => self.new_item(dir, true),
            "rename" => self.rename(path),
            "trash" => self.trash(path),
            "copy-path" => self.list.clipboard().set_text(&path.to_string_lossy()),
            "copy-relative-path" => self
                .list
                .clipboard()
                .set_text(&fileops::relative(&root, &path)),
            "open" => {
                if let Some(app) = self.app.upgrade() {
                    app.launch(&gio::File::for_path(&path).uri());
                }
            }
            "show" => self.show_in_manager(path),
            _ => {}
        }
    }

    fn show_in_manager(self: &Rc<Self>, path: PathBuf) {
        let window = self.list.root().and_downcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&path)));
            if let Err(why) = launcher
                .open_containing_folder_future(window.as_ref())
                .await
                && let Some(this) = weak.upgrade()
            {
                this.say(&format!("Couldn\u{2019}t show the file: {}", why.message()));
            }
        });
    }

    fn new_item(self: &Rc<Self>, dir: PathBuf, is_dir: bool) {
        let heading = if is_dir { "New Folder" } else { "New File" };
        let at = dir.clone();
        let prompt = Prompt {
            heading: heading.into(),
            ok: "Create".into(),
            dir,
            initial: String::new(),
            current: None,
        };
        self.ask_name(prompt, move |this, name| {
            let path = at.join(&name);
            let (made, weak) = (path.clone(), Rc::downgrade(this));
            glib::spawn_future_local(async move {
                let result =
                    gio::spawn_blocking(move || xenon_finder::fsops::create(&made, is_dir)).await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(Ok(())) => this.finish(&[at], Some(path)),
                    Ok(Err(why)) => this.say(&format!("Couldn\u{2019}t create {name}: {why}")),
                    Err(_) => {}
                }
            });
        });
    }

    fn rename(self: &Rc<Self>, path: PathBuf) {
        let (Some(dir), Some(name)) = (
            path.parent().map(Path::to_path_buf),
            path.file_name().map(|n| n.to_string_lossy().into_owned()),
        ) else {
            return;
        };
        let prompt = Prompt {
            heading: "Rename".into(),
            ok: "Rename".into(),
            dir: dir.clone(),
            initial: name.clone(),
            current: Some(name),
        };
        self.ask_name(prompt, move |this, new_name| {
            let weak = Rc::downgrade(this);
            glib::spawn_future_local(async move {
                let result = gio::File::for_path(&path)
                    .set_display_name_future(&new_name, glib::Priority::DEFAULT)
                    .await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(renamed) => {
                        let new = renamed.path().unwrap_or_else(|| dir.join(&new_name));
                        if let Some(app) = this.app.upgrade() {
                            app.paths_moved(&path, &new);
                        }
                        this.finish(&[dir], Some(new));
                    }
                    Err(why) => this.say(&format!("Couldn\u{2019}t rename: {}", why.message())),
                }
            });
        });
    }

    fn trash(self: &Rc<Self>, path: PathBuf) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = gio::File::for_path(&path)
                .trash_future(glib::Priority::DEFAULT)
                .await;
            let Some(this) = weak.upgrade() else { return };
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            match result {
                Ok(()) => this.say(&format!("Moved \u{201c}{name}\u{201d} to the trash")),
                Err(why) => this.say(&format!(
                    "Couldn\u{2019}t move \u{201c}{name}\u{201d} to the trash: {}",
                    why.message()
                )),
            }
            this.finish(
                &path
                    .parent()
                    .map(Path::to_path_buf)
                    .into_iter()
                    .collect::<Vec<_>>(),
                None,
            );
        });
    }

    /// Ask for a name for an item in `dir`; `then` gets the checked name.
    fn ask_name(self: &Rc<Self>, prompt: Prompt, then: impl FnOnce(&Rc<Self>, String) + 'static) {
        let Prompt {
            heading,
            ok,
            dir,
            initial,
            current,
        } = prompt;
        let dialog = adw::AlertDialog::new(Some(&heading), None);
        dialog.add_responses(&[("cancel", "Cancel"), ("ok", &ok)]);
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("ok", false);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::new();
        entry.set_text(&initial);
        entry.set_activates_default(true);
        let note = gtk::Label::new(None);
        note.add_css_class("error");
        note.set_xalign(0.0);
        note.set_wrap(true);
        note.set_visible(false);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.append(&entry);
        column.append(&note);
        dialog.set_extra_child(Some(&column));
        dialog.set_focus(Some(&entry));

        let check = {
            let (dir, current) = (dir.clone(), current.clone());
            move |text: &str| fileops::check_name(&dir, text, current.as_deref())
        };
        let (weak_dialog, weak_note) = (dialog.downgrade(), note.downgrade());
        let check_live = check.clone();
        entry.connect_changed(move |entry| {
            let (Some(dialog), Some(note)) = (weak_dialog.upgrade(), weak_note.upgrade()) else {
                return;
            };
            let text = entry.text();
            let quiet = text.trim().is_empty() || current.as_deref() == Some(text.trim());
            let result = check_live(&text);
            note.set_visible(!quiet && result.is_err());
            note.set_text(result.as_ref().err().map_or("", String::as_str));
            dialog.set_response_enabled("ok", result.is_ok());
        });
        let (weak, weak_entry) = (Rc::downgrade(self), entry.downgrade());
        let then = std::cell::Cell::new(Some(then));
        dialog.connect_response(None, move |_, id| {
            let (Some(this), Some(entry), "ok") = (weak.upgrade(), weak_entry.upgrade(), id) else {
                return;
            };
            if let (Ok(name), Some(then)) = (check(&entry.text()), then.take()) {
                glib::idle_add_local_once(move || then(&this, name));
            }
        });
        dialog.present(self.list.root().as_ref());
        // Rename selects the name without its extension.
        let stem = fileops::stem_chars(&initial) as i32;
        glib::idle_add_local_once(move || {
            entry.grab_focus();
            if stem > 0 {
                entry.select_region(0, stem);
            }
        });
    }
}

/// What a name prompt asks: titles, the folder the item lands in, the text to
/// start with and, when renaming, the current name.
struct Prompt {
    heading: String,
    ok: String,
    dir: PathBuf,
    initial: String,
    current: Option<String>,
}
