//! The Files section of the sidebar: the workspace folder as a stock
//! `GtkListView` over a `GtkTreeListModel`. Each folder is a `gio::ListStore` of
//! `gio::FileInfo` that is read on a worker thread when the folder is opened;
//! only open folders are watched (a cap keeps the number of watchers bounded).
//! Like the GPUI tree it lists every entry, folders first.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib, pango};
use xenon_finder::tree::TreeEntry;

use crate::app::App;
use crate::model::Fold;
use crate::sidebar_ui::{Row, Section};

/// Most folders watched at once; deeper expansions are not watched.
const MAX_WATCHED: usize = 48;
/// Coalesce a burst of file events into one reload.
const RELOAD_DELAY: Duration = Duration::from_millis(250);
const FILE_ATTR: &str = "xenon::file";

mod watch;

type Stores = Rc<RefCell<HashMap<PathBuf, gio::ListStore>>>;

pub(crate) struct FilesView {
    pub root: gtk::Box,
    section: Section,
    list: gtk::ListView,
    tree: gtk::TreeListModel,
    selection: gtk::SingleSelection,
    root_store: gio::ListStore,
    /// The children of each folder the tree has asked about.
    stores: Stores,
    root_dir: RefCell<Option<PathBuf>>,
    app: Weak<App>,
    /// Rows on screen, so the open-file mark can be redrawn.
    bound: Rc<RefCell<Vec<gtk::ListItem>>>,
    monitors: RefCell<HashMap<PathBuf, gio::FileMonitor>>,
    dirty: RefCell<HashSet<PathBuf>>,
    reload_armed: Cell<bool>,
    /// File shown in the active editor tab.
    open_file: Rc<RefCell<Option<PathBuf>>>,
    /// Folder to open and select once its parents are read (`reveal`).
    reveal_target: RefCell<Option<PathBuf>>,
    /// Set while re-opening folders after a reload, which must not read again.
    restoring: Cell<bool>,
    /// Bumped when the root changes, so a late directory read is dropped.
    epoch: Cell<u64>,
}

fn path_of(item: &glib::Object) -> Option<PathBuf> {
    item.downcast_ref::<gio::FileInfo>()?
        .attribute_object(FILE_ATTR)?
        .downcast::<gio::File>()
        .ok()?
        .path()
}

fn info_for(entry: &TreeEntry) -> gio::FileInfo {
    let info = gio::FileInfo::new();
    info.set_display_name(&entry.name);
    info.set_file_type(if entry.is_dir {
        gio::FileType::Directory
    } else {
        gio::FileType::Regular
    });
    info.set_is_symlink(entry.is_symlink);
    info.set_attribute_object(FILE_ATTR, &gio::File::for_path(&entry.path));
    info
}

fn is_dir(item: &glib::Object) -> bool {
    item.downcast_ref::<gio::FileInfo>()
        .is_some_and(|i| i.file_type() == gio::FileType::Directory)
}

fn file_icon(info: &gio::FileInfo, path: &Path) -> &'static str {
    if info.file_type() == gio::FileType::Directory {
        return "folder-symbolic";
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
    {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
            "image-x-generic-symbolic"
        }
        "sh" | "bash" | "zsh" | "fish" => "utilities-terminal-symbolic",
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "7z" | "rar" => "package-x-generic-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

fn tree_row(item: Option<glib::Object>) -> Option<gtk::TreeListRow> {
    item?.downcast().ok()
}

fn row_path(row: &gtk::TreeListRow) -> Option<PathBuf> {
    path_of(&row.item()?)
}

impl FilesView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let root_store = gio::ListStore::new::<gio::FileInfo>();
        let stores: Stores = Rc::default();
        let tree = gtk::TreeListModel::new(root_store.clone(), false, false, {
            let stores = stores.clone();
            move |item| {
                if !is_dir(item) {
                    return None;
                }
                let store = stores
                    .borrow_mut()
                    .entry(path_of(item)?)
                    .or_insert_with(gio::ListStore::new::<gio::FileInfo>)
                    .clone();
                Some(store.upcast())
            }
        });
        let selection = gtk::SingleSelection::new(Some(tree.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let bound: Rc<RefCell<Vec<gtk::ListItem>>> = Rc::default();
        let open_file: Rc<RefCell<Option<PathBuf>>> = Rc::default();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let expander = gtk::TreeExpander::new();
            expander.set_child(Some(&Row::new().root));
            item.set_child(Some(&expander));
        });
        factory.connect_bind({
            let (bound, open) = (bound.clone(), open_file.clone());
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                bound.borrow_mut().push(item.clone());
                draw_item(item, open.borrow().as_deref());
            }
        });
        factory.connect_unbind({
            let bound = bound.clone();
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                bound.borrow_mut().retain(|b| b != item);
                if let Some(expander) = item.child().and_downcast::<gtk::TreeExpander>() {
                    expander.set_list_row(None);
                }
            }
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list.set_single_click_activate(true);
        list.add_css_class("navigation-sidebar");
        list.add_css_class("xfiles");
        let section = Section::new("FILES", Fold::Files, &list, None, &app);
        let this = Rc::new(Self {
            root: section.root.clone(),
            section,
            list,
            tree,
            selection,
            root_store,
            stores,
            root_dir: RefCell::new(None),
            app,
            bound,
            monitors: RefCell::default(),
            dirty: RefCell::default(),
            reload_armed: Cell::new(false),
            open_file,
            reveal_target: RefCell::new(None),
            restoring: Cell::new(false),
            epoch: Cell::new(0),
        });
        this.section.sep.set_visible(true);
        this.wire();
        this
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        // Act after the click or key has finished: changing the tree or the
        // panes inside the list's own activate handler crashes GTK.
        self.list.connect_activate(move |_, pos| {
            let Some(this) = weak.upgrade() else { return };
            let Some(row) = tree_row(this.tree.item(pos)) else {
                return;
            };
            let weak = Rc::downgrade(&this);
            glib::idle_add_local_once(move || {
                let Some(this) = weak.upgrade() else { return };
                if row.is_expandable() {
                    row.set_expanded(!row.is_expanded());
                } else if let (Some(path), Some(app)) = (row_path(&row), this.app.upgrade()) {
                    let ws = app.model.borrow().active();
                    if let Some(ws) = ws {
                        app.open_editor(ws, &path);
                    }
                }
            });
        });
        // Folders are read when opened and forgotten when closed, however the
        // row was opened (arrow, key or click).
        let weak = Rc::downgrade(self);
        self.tree.connect_items_changed(move |tree, pos, _, added| {
            for i in pos..pos + added {
                let Some(row) =
                    tree_row(tree.item(i)).filter(|r| r.item().is_some_and(|o| is_dir(&o)))
                else {
                    continue;
                };
                let weak = weak.clone();
                row.connect_expanded_notify(move |row| {
                    let Some(this) = weak.upgrade() else { return };
                    if let Some(path) = row_path(row) {
                        this.expanded_changed(&path, row.is_expanded());
                    }
                });
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let handled = match key {
                gdk::Key::Right | gdk::Key::l => this.step_in(),
                gdk::Key::Left | gdk::Key::h => this.step_out(),
                gdk::Key::Escape => {
                    if let Some(app) = this.app.upgrade() {
                        app.focus_terminal();
                    }
                    true
                }
                _ => false,
            };
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.list.add_controller(keys);
        // Scripts read `files-state`: keep it current when focus or selection moves.
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_enter({
            let weak = weak.clone();
            move |_| publish(&weak)
        });
        focus.connect_leave({
            let weak = weak.clone();
            move |_| publish(&weak)
        });
        self.list.add_controller(focus);
        self.selection
            .connect_selection_changed(move |_, _, _| publish(&weak));
    }

    fn selected_row(&self) -> Option<gtk::TreeListRow> {
        tree_row(self.selection.selected_item())
    }

    fn selected_path(&self) -> Option<PathBuf> {
        row_path(&self.selected_row()?)
    }

    /// Right: open a closed folder, or step into an open one.
    fn step_in(&self) -> bool {
        let Some(row) = self.selected_row().filter(|r| r.is_expandable()) else {
            return false;
        };
        if row.is_expanded() {
            self.focus_index(self.selection.selected() + 1);
        } else {
            row.set_expanded(true);
        }
        true
    }

    /// Left: close an open folder, or step out to the parent.
    fn step_out(&self) -> bool {
        let Some(row) = self.selected_row() else {
            return false;
        };
        if row.is_expanded() {
            row.set_expanded(false);
        } else if let Some(path) = row.parent().and_then(|p| row_path(&p)) {
            self.select_path(&path, true);
        }
        true
    }

    fn focus_index(&self, index: u32) {
        if index < self.tree.n_items() {
            self.selection.set_selected(index);
            self.list
                .scroll_to(index, gtk::ListScrollFlags::FOCUS, None);
        }
    }

    /// Show `root` (the workspace folder), or nothing.
    pub(crate) fn set_root(self: &Rc<Self>, root: Option<PathBuf>) {
        if *self.root_dir.borrow() == root {
            return;
        }
        self.epoch.set(self.epoch.get() + 1);
        self.monitors.borrow_mut().clear();
        self.reveal_target.take();
        self.root_store.remove_all();
        self.stores.borrow_mut().clear();
        *self.root_dir.borrow_mut() = root.clone();
        self.root.set_visible(root.is_some());
        if let Some(root) = root {
            self.load(&root);
        }
        self.publish();
    }

    fn select_path(&self, path: &Path, focus: bool) {
        if let Some((i, _)) = self.find(path) {
            self.selection.set_selected(i);
            if focus {
                self.list.scroll_to(i, gtk::ListScrollFlags::FOCUS, None);
            }
        }
    }

    /// Draw the fold the model decided.
    pub(crate) fn show_fold(&self, folded: bool) {
        self.section.show_fold(folded);
    }

    /// Put the keyboard in the tree.
    pub(crate) fn focus(&self) {
        if let Some(app) = self.app.upgrade() {
            app.set_fold(Fold::Files, false);
        }
        let at = if self.selection.selected() < self.tree.n_items() {
            self.selection.selected()
        } else {
            0
        };
        self.focus_index(at);
        self.publish();
    }

    pub(super) fn publish(&self) {
        if let Some(app) = self.app.upgrade() {
            app.publish_files();
        }
    }

    pub(crate) fn has_rows(&self) -> bool {
        self.tree.n_items() > 0
    }
}

fn publish(weak: &Weak<FilesView>) {
    if let Some(this) = weak.upgrade() {
        this.publish();
    }
}

/// Fill one bound list item from its tree row.
fn draw_item(item: &gtk::ListItem, open: Option<&Path>) {
    let (Some(expander), Some(row)) = (
        item.child().and_downcast::<gtk::TreeExpander>(),
        tree_row(item.item()),
    ) else {
        return;
    };
    expander.set_list_row(Some(&row));
    let (Some(info), Some(path)) = (row.item().and_downcast::<gio::FileInfo>(), row_path(&row))
    else {
        return;
    };
    let Some(line) = expander.child().and_then(|c| Row::of(&c)) else {
        return;
    };
    line.set(Some(file_icon(&info, &path)), &info.display_name(), "");
    line.set_open(open == Some(path.as_path()));
    let attrs = pango::AttrList::new();
    if info.is_symlink() {
        attrs.insert(pango::AttrInt::new_style(pango::Style::Italic));
    }
    line.title.set_attributes(Some(&attrs));
    expander.set_tooltip_text(Some(&path.to_string_lossy()));
}
