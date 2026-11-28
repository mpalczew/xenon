use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::app::defer;
use crate::commands::Cmd;
use crate::dialogs::{Prompt, ask_name};
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_finder::fileops;

use super::{FilesView, path_of, row_path};

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
    let view = gio::Menu::new();
    let hidden = Cmd::ShowHiddenFiles.menu_item();
    hidden.set_attribute_value("accel", Some(&"<Control>h".to_variant()));
    view.append_item(&hidden);
    menu.append_section(None, &view);
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
                defer(&weak, move |this| this.run_action(name));
            });
            group.add_action(&action);
        }
        self.list.insert_action_group("files", Some(&group));

        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        // sync: event-time read: reads the row under the pointer now, before the list changes; the popup is deferred.
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
            defer(&weak, move |this| {
                this.popup(target, at);
            });
        });
        self.list.add_controller(click);

        self.wire_drop();
    }

    fn popup(self: &Rc<Self>, target: Option<Target>, at: gdk::Rectangle) {
        if let Some(target) = &target {
            self.select_path(&target.path, false);
        }
        let menu = gtk::PopoverMenu::from_model(Some(&menu_model(target.is_some())));
        *self.ctx.borrow_mut() = target;
        menu.set_has_arrow(false);
        menu.set_position(gtk::PositionType::Right);
        menu.set_valign(gtk::Align::Start);
        menu.set_parent(&self.list);
        menu.set_pointing_to(Some(&at));
        menu.connect_closed(|menu| {
            let menu = menu.clone();
            glib::idle_add_local_once(move || menu.unparent());
        });
        menu.connect_map(|menu| {
            let menu = menu.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                let (min, nat, _, _) = menu.measure(gtk::Orientation::Vertical, -1);
                let surface = menu.native().and_then(|n| n.surface());
                let monitor = surface
                    .as_ref()
                    .and_then(|s| s.display().monitor_at_surface(s))
                    .map(|m| m.geometry());
                eprintln!(
                    "[diag] files menu: want min={min} nat={nat} got={} surface={:?} monitor={:?}",
                    menu.height(),
                    surface.map(|s| (s.width(), s.height())),
                    monitor.map(|g| (g.width(), g.height())),
                );
            });
        });
        menu.popup();
    }

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
        defer(&weak, move |this| {
            this.do_action(action, target);
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
            title: heading,
            detail: None,
            ok: "Create",
            initial: String::new(),
        };
        self.ask_file_name(prompt, dir, None, move |this, name| {
            let path = at.join(&name);
            let (made, weak) = (path.clone(), Rc::downgrade(this));
            glib::spawn_future_local(async move {
                let result =
                    gio::spawn_blocking(move || xenon_finder::fsops::create(&made, is_dir)).await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(Ok(())) => this.reveal(&path),
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
            title: "Rename",
            detail: None,
            ok: "Rename",
            initial: name.clone(),
        };
        self.ask_file_name(
            prompt,
            dir.clone(),
            Some(name.clone()),
            move |this, new_name| {
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
                            this.reveal(&new);
                        }
                        Err(why) => this.say(&format!("Couldn\u{2019}t rename: {}", why.message())),
                    }
                });
            },
        );
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
        });
    }

    fn ask_file_name(
        self: &Rc<Self>,
        prompt: Prompt,
        dir: PathBuf,
        current: Option<String>,
        then: impl FnOnce(&Rc<Self>, String) + 'static,
    ) {
        let (weak, list) = (Rc::downgrade(self), self.list.clone());
        let check = move |text: &str| fileops::check_name(&dir, text, current.as_deref());
        glib::spawn_future_local(async move {
            let name = ask_name(&list, prompt, check).await;
            if let (Some(name), Some(this)) = (name, weak.upgrade()) {
                then(&this, name);
            }
        });
    }
}
