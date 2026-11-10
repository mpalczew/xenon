use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib, pango};

use crate::app::{App, defer};
use crate::model::Fold;
use crate::sidebar_ui::{Row, Section};

const ATTRS: &str = "standard::name,standard::display-name,standard::type,standard::is-symlink,standard::symbolic-icon";

mod drop;
mod ops;
mod watch;

use drop::OwnerCell;
use ops::Target;

pub(crate) struct FilesView {
    pub root: gtk::Box,
    section: Section,
    list: gtk::ListView,
    tree: gtk::TreeListModel,
    selection: gtk::SingleSelection,
    root_list: gtk::DirectoryList,
    root_dir: RefCell<Option<PathBuf>>,
    app: Weak<App>,
    bound: Rc<RefCell<Vec<gtk::ListItem>>>,
    open_file: Rc<RefCell<Option<PathBuf>>>,
    reveal_target: RefCell<Option<PathBuf>>,
    menu: gtk::PopoverMenu,
    ctx: RefCell<Option<Target>>,
    drag: RefCell<Vec<PathBuf>>,
}

fn path_of(item: &glib::Object) -> Option<PathBuf> {
    item.downcast_ref::<gio::FileInfo>()?
        .attribute_object("standard::file")?
        .downcast::<gio::File>()
        .ok()?
        .path()
}

fn is_dir(item: &glib::Object) -> bool {
    item.downcast_ref::<gio::FileInfo>()
        .is_some_and(|i| i.file_type() == gio::FileType::Directory)
}

/// A live, folders-first, case-insensitive listing of `dir`.
fn listing(dir: Option<&gio::File>) -> (gtk::DirectoryList, gtk::SortListModel) {
    let list = gtk::DirectoryList::new(Some(ATTRS), dir);
    list.set_monitored(true);
    let sorter = gtk::CustomSorter::new(|a, b| {
        let key = |o: &glib::Object| {
            let info = o.downcast_ref::<gio::FileInfo>()?;
            Some((!is_dir(o), info.display_name().to_lowercase()))
        };
        key(a).cmp(&key(b)).into()
    });
    let sorted = gtk::SortListModel::new(Some(list.clone()), Some(sorter));
    (list, sorted)
}

fn tree_row(item: Option<glib::Object>) -> Option<gtk::TreeListRow> {
    item?.downcast().ok()
}

fn row_path(row: &gtk::TreeListRow) -> Option<PathBuf> {
    path_of(&row.item()?)
}

impl FilesView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let (root_list, root_sorted) = listing(None);
        let tree = gtk::TreeListModel::new(root_sorted, false, false, |item| {
            if !is_dir(item) {
                return None;
            }
            let (_, sorted) = listing(Some(&gio::File::for_path(path_of(item)?)));
            Some(sorted.upcast())
        });
        let selection = gtk::SingleSelection::new(Some(tree.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let bound: Rc<RefCell<Vec<gtk::ListItem>>> = Rc::default();
        let open_file: Rc<RefCell<Option<PathBuf>>> = Rc::default();
        let owner = OwnerCell::default();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup({
            let owner = owner.clone();
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                let expander = gtk::TreeExpander::new();
                expander.set_child(Some(&Row::new().root));
                FilesView::row_events(&owner, &expander);
                item.set_child(Some(&expander));
            }
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
            root_list,
            root_dir: RefCell::new(None),
            app,
            bound,
            open_file,
            reveal_target: RefCell::new(None),
            menu: gtk::PopoverMenu::from_model(None::<&gio::MenuModel>),
            ctx: RefCell::new(None),
            drag: RefCell::default(),
        });
        *owner.0.borrow_mut() = Rc::downgrade(&this);
        this.section.sep.set_visible(false);
        this.wire();
        this.wire_ops();
        this
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        // Act after the click or key has finished: changing the tree or the
        // panes inside the list's own activate handler crashes GTK.
        // sync: event-time read: `tree_row` only reads the item; the work is deferred.
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
        let weak = Rc::downgrade(self);
        // Rows arrive as folders finish loading; keep revealing until the target shows.
        self.tree.connect_items_changed(move |_, _, _, added| {
            if added == 0 {
                return;
            }
            defer(&weak, move |this| {
                this.advance_reveal();
            });
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        // sync: answer or ordering before return: must say at once whether it consumed the key; the tree steps act on the selection it was meant for.
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
                gdk::Key::F2 => this.act_on_selected("rename"),
                gdk::Key::Delete | gdk::Key::KP_Delete => this.act_on_selected("trash"),
                gdk::Key::Menu => this.popup_selected(),
                gdk::Key::F10 if state.contains(gdk::ModifierType::SHIFT_MASK) => {
                    this.popup_selected()
                }
                gdk::Key::Escape => {
                    defer(&this.app, move |app| app.focus_terminal());
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
    }

    fn selected_row(&self) -> Option<gtk::TreeListRow> {
        tree_row(self.selection.selected_item())
    }

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

    pub(crate) fn set_root(self: &Rc<Self>, root: Option<PathBuf>) {
        if *self.root_dir.borrow() == root {
            return;
        }
        self.reveal_target.take();
        *self.root_dir.borrow_mut() = root.clone();
        self.root.set_visible(root.is_some());
        self.root_list
            .set_file(root.as_deref().map(gio::File::for_path).as_ref());
    }

    fn select_path(&self, path: &Path, focus: bool) {
        if let Some((i, _)) = self.find(path) {
            self.selection.set_selected(i);
            if focus {
                self.list.scroll_to(i, gtk::ListScrollFlags::FOCUS, None);
            }
        }
    }

    pub(crate) fn show_fold(&self, folded: bool) {
        self.section.show_fold(folded);
    }

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
    }

    pub(crate) fn has_rows(&self) -> bool {
        self.tree.n_items() > 0
    }
}

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
    line.set(Some("text-x-generic-symbolic"), &info.display_name(), "");
    if let Some(icon) = info.symbolic_icon() {
        line.icon.set_from_gicon(&icon);
    }
    line.set_open(open == Some(path.as_path()));
    let attrs = pango::AttrList::new();
    if info.is_symlink() {
        attrs.insert(pango::AttrInt::new_style(pango::Style::Italic));
    }
    line.title.set_attributes(Some(&attrs));
    expander.set_tooltip_text(Some(&path.to_string_lossy()));
}
