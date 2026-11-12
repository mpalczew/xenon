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
mod ignores;
mod ops;
mod watch;

use drop::OwnerCell;
use ignores::Ignores;
use ops::Target;

pub(crate) struct FilesView {
    pub root: gtk::Box,
    section: Section,
    list: gtk::ListView,
    tree: gtk::TreeListModel,
    selection: gtk::SingleSelection,
    root_list: gtk::DirectoryList,
    root_dir: RefCell<Option<PathBuf>>,
    ignores: Rc<Ignores>,
    app: Weak<App>,
    bound: Rc<RefCell<Vec<gtk::ListItem>>>,
    open_file: Rc<RefCell<Option<PathBuf>>>,
    reveal_target: RefCell<Option<PathBuf>>,
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

/// A live, folders-first, case-insensitive listing of `dir` without ignored entries.
fn listing(
    dir: Option<&gio::File>,
    ignores: &Rc<Ignores>,
) -> (gtk::DirectoryList, gtk::FilterListModel) {
    let list = gtk::DirectoryList::new(Some(ATTRS), dir);
    list.set_monitored(true);
    ignores.watch(&list);
    let sorter = gtk::CustomSorter::new(|a, b| {
        let key = |o: &glib::Object| {
            let info = o.downcast_ref::<gio::FileInfo>()?;
            Some((!is_dir(o), info.display_name().to_lowercase()))
        };
        key(a).cmp(&key(b)).into()
    });
    let sorted = gtk::SortListModel::new(Some(list.clone()), Some(sorter));
    let filtered = gtk::FilterListModel::new(Some(sorted), Some(ignores.filter.clone()));
    (list, filtered)
}

fn tree_row(item: Option<glib::Object>) -> Option<gtk::TreeListRow> {
    item?.downcast().ok()
}

fn row_path(row: &gtk::TreeListRow) -> Option<PathBuf> {
    path_of(&row.item()?)
}

impl FilesView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let bound: Rc<RefCell<Vec<gtk::ListItem>>> = Rc::default();
        let open_file: Rc<RefCell<Option<PathBuf>>> = Rc::default();
        let show_hidden = xenon_store::load_settings().is_ok_and(|s| s.show_hidden_files);
        let ignores = Ignores::new(show_hidden, bound.clone(), open_file.clone());
        let (root_list, root_sorted) = listing(None, &ignores);
        let tree = gtk::TreeListModel::new(root_sorted, false, false, {
            let ignores = ignores.clone();
            move |item| {
                if !is_dir(item) {
                    return None;
                }
                let (_, sorted) = listing(Some(&gio::File::for_path(path_of(item)?)), &ignores);
                Some(sorted.upcast())
            }
        });
        let selection = gtk::SingleSelection::new(Some(tree.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
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
            let (bound, ignores) = (bound.clone(), ignores.clone());
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                bound.borrow_mut().push(item.clone());
                draw_item(item, ignores.open.borrow().as_deref(), &ignores);
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
            ignores,
            app,
            bound,
            open_file,
            reveal_target: RefCell::new(None),
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
            let ctrl_h = key == gdk::Key::h && state == gdk::ModifierType::CONTROL_MASK;
            if !ctrl_h
                && state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let handled = match key {
                gdk::Key::h if ctrl_h => {
                    defer(&weak, |this| this.toggle_hidden());
                    true
                }
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
        *self.ignores.root.borrow_mut() = root.clone();
        self.ignores.refresh();
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

    /// Flip "Show Hidden Files": filter, dimming, the menu check and the saved setting.
    fn toggle_hidden(&self) {
        self.set_hidden(!self.ignores.show_hidden.get());
    }

    pub(crate) fn set_hidden(&self, on: bool) {
        self.ignores.set_show_hidden(on);
        if let Some(app) = self.app.upgrade()
            && let Some(action) = app.gtk.lookup_action("show-hidden-files")
            && let Ok(action) = action.downcast::<gio::SimpleAction>()
        {
            action.set_state(&on.to_variant());
        }
        if let Err(error) = xenon_store::update_settings(|s| s.show_hidden_files = on) {
            eprintln!("[xg] save settings: {error}");
        }
    }

    pub(crate) fn has_rows(&self) -> bool {
        self.tree.n_items() > 0
    }
}

/// One monochrome icon per file-type group, the same under every icon theme.
fn file_icon(name: &Path) -> &'static str {
    let file = name.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = file.to_lowercase();
    let key = match lower.rsplit_once('.') {
        Some((_, ext)) => ext,
        None => lower.as_str(),
    };
    match key {
        "md" | "markdown" | "mdx" => "xenon-file-markdown-symbolic",
        "ts" | "tsx" | "js" | "mjs" | "cjs" | "swift" | "rs" | "py" | "cs" | "rb" | "astro"
        | "go" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "lua" | "zig" => {
            "xenon-file-code-symbolic"
        }
        "sh" | "bash" | "zsh" | "fish" | "makefile" | "dockerfile" | "fastfile" | "appfile"
        | "justfile" => "xenon-file-shell-symbolic",
        "json" | "jsonc" | "toml" | "yml" | "yaml" | "xml" | "nix" | "tf" | "plist" | "lock"
        | "gradle" | "bazel" | "bzl" | "service" | "ini" | "conf" | "env" | "gitignore"
        | "gitattributes" | "editorconfig" => "xenon-file-config-symbolic",
        "sql" | "sqlite" | "db" => "xenon-file-database-symbolic",
        "html" | "htm" | "css" | "scss" => "xenon-file-web-symbolic",
        "csv" | "tsv" | "xlsx" | "xlsm" | "xls" | "ods" => "x-office-spreadsheet-symbolic",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
            "image-x-generic-symbolic"
        }
        "pdf" | "azw" | "mobi" | "epub" | "doc" | "docx" | "odt" => "x-office-document-symbolic",
        "wav" | "mp3" | "flac" | "ogg" | "m4a" => "audio-x-generic-symbolic",
        "mp4" | "mkv" | "webm" | "mov" => "video-x-generic-symbolic",
        "woff" | "woff2" | "ttf" | "otf" => "font-x-generic-symbolic",
        "zip" | "tar" | "gz" | "xz" | "zst" | "7z" => "package-x-generic-symbolic",
        "bin" | "so" | "o" | "a" => "application-x-executable-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

fn draw_item(item: &gtk::ListItem, open: Option<&Path>, ignores: &Ignores) {
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
    if is_dir(info.upcast_ref()) {
        // Keep only monochrome names so a theme's color icon never slips in.
        if let Some(icon) = info.symbolic_icon().and_downcast::<gio::ThemedIcon>() {
            let mut names: Vec<_> = icon
                .names()
                .into_iter()
                .filter(|n| n.ends_with("-symbolic"))
                .collect();
            names.push("text-x-generic-symbolic".into());
            let names: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
            line.icon
                .set_from_gicon(&gio::ThemedIcon::from_names(&names));
        }
    } else {
        line.icon.set_icon_name(Some(file_icon(&info.name())));
    }
    if ignores.hidden(info.upcast_ref()) {
        expander.add_css_class("dim-label");
    } else {
        expander.remove_css_class("dim-label");
    }
    line.set_open(open == Some(path.as_path()));
    let attrs = pango::AttrList::new();
    if info.is_symlink() {
        attrs.insert(pango::AttrInt::new_style(pango::Style::Italic));
    }
    line.title.set_attributes(Some(&attrs));
    expander.set_tooltip_text(Some(&path.to_string_lossy()));
}
