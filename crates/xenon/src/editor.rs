use std::cell::{Cell, OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use sourceview::prelude::*;
use xenon_core::{TabId, WorkspaceId};
use xenon_settings::{MarkdownMode, prose_file_prefs, set_prose_file_prefs};

use crate::app::App;
use crate::prose::{self, Prose};
use crate::worklist_list::WorklistList;

mod list;
mod switch;

const MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) struct EditorView {
    pub root: gtk::Widget,
    pub view: sourceview::View,
    pub buffer: sourceview::Buffer,
    pub path: PathBuf,
    loading: Cell<bool>,
    vim: RefCell<Option<(sourceview::VimIMContext, gtk::EventControllerKey)>>,
    pub(crate) prose: Option<Rc<Prose>>,
    mode: Cell<MarkdownMode>,
    switch: Option<adw::ToggleGroup>,
    syncing: Cell<bool>,
    list: OnceCell<Rc<WorklistList>>,
    stack: Option<gtk::Stack>,
    app: Weak<App>,
    scroller: gtk::ScrolledWindow,
    bar: gtk::Box,
    bar_text: gtk::Label,
    bar_actions: gtk::Box,
    monitor: RefCell<Option<gio::FileMonitor>>,
    sync_pending: Cell<bool>,
    stale: Cell<bool>,
    file: sourceview::File,
}

const DISK_SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

pub(crate) fn is_html(path: &Path) -> bool {
    gio::content_type_guess(Some(path), None::<&[u8]>).0 == "text/html"
}

fn browser_root(path: &Path) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if !is_html(path) {
        return root;
    }
    let button = gtk::Button::with_label("Open in Browser");
    button.set_action_name(Some("app.open-path-default"));
    button.set_action_target_value(Some(&path.to_string_lossy().to_variant()));
    button.set_tooltip_text(Some("Open in the default browser"));
    button.set_focusable(false);
    button.set_halign(gtk::Align::End);
    button.add_css_class("flat");
    root.append(&button);
    root
}

fn disk_bar() -> (gtk::Box, gtk::Label, gtk::Box) {
    let text = gtk::Label::new(None);
    text.set_hexpand(true);
    text.set_xalign(0.0);
    text.set_wrap(true);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("toolbar");
    bar.append(&text);
    bar.append(&actions);
    bar.set_visible(false);
    (bar, text, actions)
}

fn disk_file(path: &Path) -> sourceview::File {
    sourceview::File::builder()
        .location(&gio::File::for_path(path))
        .build()
}

fn limits(regular: bool, len: u64) -> Result<(), String> {
    if !regular {
        return Err("not a regular file".into());
    }
    if len > MAX_BYTES {
        return Err("larger than 4 MB".into());
    }
    Ok(())
}

pub(crate) fn check(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    limits(meta.is_file(), meta.len())
}

impl EditorView {
    pub(crate) fn new(app: &Rc<App>, ws: WorkspaceId, tab: TabId, path: &Path) -> Rc<Self> {
        let buffer = sourceview::Buffer::new(None);
        buffer.set_highlight_syntax(true);
        if let Some(lang) =
            sourceview::LanguageManager::default().guess_language(Some(path), None::<&str>)
        {
            buffer.set_language(Some(&lang));
        }
        let view = sourceview::View::with_buffer(&buffer);
        view.add_css_class("xeditor");
        view.set_editable(false);
        view.set_monospace(true);
        view.set_hexpand(true);
        view.set_vexpand(true);
        view.set_top_margin(6);
        view.set_bottom_margin(6);
        view.set_left_margin(8);
        view.set_right_margin(12);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&view));
        let prose = prose::is_markdown(path).then(|| Prose::new(app, ws, &view, &scroller, path));
        let mode = prose
            .as_ref()
            .map_or(MarkdownMode::Source, |_| prose_file_prefs(path).mode);
        let (bar, bar_text, bar_actions) = disk_bar();
        let root = browser_root(path);
        root.append(&bar);
        let switch = prose.as_ref().map(|_| switch::add_row(path, &root));
        let stack = list::pages(path, &root, &scroller);
        let editor = Rc::new(Self {
            root: root.upcast(),
            scroller,
            bar,
            bar_text,
            bar_actions,
            monitor: RefCell::new(None),
            sync_pending: Cell::new(false),
            stale: Cell::new(false),
            file: disk_file(path),
            view,
            buffer,
            path: path.to_path_buf(),
            loading: Cell::new(false),
            vim: RefCell::new(None),
            prose,
            mode: Cell::new(mode),
            switch,
            syncing: Cell::new(false),
            list: OnceCell::new(),
            stack,
            app: Rc::downgrade(app),
        });
        editor.attach_list();
        editor.connect_switch();
        editor.sync_switch();
        editor.restyle(app);
        let weak: Weak<App> = Rc::downgrade(app);
        {
            let weak = weak.clone();
            let this = Rc::downgrade(&editor);
            editor.buffer.connect_modified_changed(move |b| {
                if let (Some(app), Some(_)) = (weak.upgrade(), this.upgrade()) {
                    let modified = b.is_modified();
                    glib::idle_add_local_once(move || app.editor_modified(ws, tab, modified));
                }
            });
        }
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = Rc::downgrade(&editor);
        // sync: answer or ordering before return: Return and Alt+1/3 must say at once whether the key was consumed (Ctrl+S is deferred).
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = this.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = state.contains(gdk::ModifierType::ALT_MASK);
            if ctrl && matches!(key, gdk::Key::s | gdk::Key::S) {
                let weak = weak.clone();
                glib::idle_add_local_once(move || this.save(&weak));
                return glib::Propagation::Stop;
            }
            let handled = match key {
                gdk::Key::_1 if alt => this.set_mode(MarkdownMode::Prose),
                gdk::Key::_3 if alt => this.set_mode(MarkdownMode::Source),
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    let plain = state.is_empty() && this.vim.borrow().is_none();
                    this.prose.as_ref().is_some_and(|p| p.enter(ctrl, plain))
                }
                _ => false,
            };
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        editor.root.add_controller(keys);
        editor.buffer.set_modified(false);
        editor.watch_disk();
        editor.load();
        editor
    }

    fn watch_disk(self: &Rc<Self>) {
        let file = gio::File::for_path(&self.path);
        let Ok(monitor) = file.monitor_file(
            gio::FileMonitorFlags::WATCH_MOVES,
            None::<&gio::Cancellable>,
        ) else {
            return;
        };
        let this = Rc::downgrade(self);
        monitor.connect_changed(move |_, _, _, _| {
            let Some(this) = this.upgrade() else { return };
            if this.sync_pending.replace(true) {
                return;
            }
            let this = Rc::downgrade(&this);
            glib::timeout_add_local_once(DISK_SETTLE, move || {
                if let Some(this) = this.upgrade() {
                    this.sync_pending.set(false);
                    this.sync_from_disk();
                    if let Some(list) = this.list.get()
                        && this.list_showing()
                    {
                        list.refresh();
                    }
                }
            });
        });
        *self.monitor.borrow_mut() = Some(monitor);
    }

    fn sync_from_disk(self: &Rc<Self>) {
        if self.loading.get() {
            self.stale.set(true);
            return;
        }
        self.file.check_file_on_disk();
        if self.file.is_deleted() {
            self.show_bar(
                "File was deleted on disk. Your buffer is still open.",
                false,
            );
            self.buffer.set_modified(true);
        } else if !self.file.is_externally_modified() {
            self.bar.set_visible(false);
        } else if self.buffer.is_modified() {
            self.show_bar(
                "File changed on disk. Keep your edits, or reload from disk?",
                true,
            );
        } else {
            self.load();
        }
    }

    async fn read(&self) -> Result<(), String> {
        let info = gio::File::for_path(&self.path)
            .query_info_future(
                "standard::type,standard::size",
                gio::FileQueryInfoFlags::NONE,
                glib::Priority::DEFAULT,
            )
            .await
            .map_err(|e| e.message().to_string())?;
        limits(
            info.file_type() == gio::FileType::Regular,
            info.size() as u64,
        )?;
        let loader = sourceview::FileLoader::new(&self.buffer, &self.file);
        loader
            .load_future(glib::Priority::DEFAULT)
            .0
            .await
            .map_err(|e| {
                if e.matches(sourceview::FileLoaderError::ConversionFallback) {
                    "xenon cannot edit this binary file".into()
                } else {
                    e.message().to_string()
                }
            })
    }

    fn load(self: &Rc<Self>) {
        if self.loading.replace(true) {
            self.stale.set(true);
            return;
        }
        let cursor = self.buffer.iter_at_offset(self.buffer.cursor_position());
        let (line, col) = (cursor.line(), cursor.line_offset());
        let scroll = self.scroller.vadjustment().value();
        // The loader deletes every mark; the Vim input method keeps its own and
        // would delete them again, so it starts over on the new text.
        let vim = self.vim.borrow().is_some();
        if vim {
            self.detach_vim();
        }
        let this = self.clone();
        glib::spawn_future_local(async move {
            let result = this.read().await;
            if let Err(why) = &result {
                this.buffer.begin_irreversible_action();
                this.buffer.set_text(&format!("{why}\n"));
                this.buffer.end_irreversible_action();
            } else {
                this.view.set_editable(true);
            }
            this.buffer.set_modified(false);
            this.loading.set(result.is_err());
            if let Some(prose) = &this.prose {
                prose.refit_column();
            }
            let at = this
                .buffer
                .iter_at_line_offset(line, col)
                .unwrap_or_else(|| this.buffer.end_iter());
            this.buffer.place_cursor(&at);
            if vim {
                this.attach_vim(&this.app.clone());
            }
            this.bar.set_visible(false);
            let adjustment = this.scroller.vadjustment();
            glib::idle_add_local_once(move || adjustment.set_value(scroll));
            if result.is_ok() && this.stale.replace(false) {
                this.sync_from_disk();
            }
        });
    }

    fn show_bar(self: &Rc<Self>, message: &str, actions: bool) {
        self.bar_text.set_text(message);
        while let Some(child) = self.bar_actions.first_child() {
            self.bar_actions.remove(&child);
        }
        if actions {
            let keep = gtk::Button::with_label("Keep mine");
            let this = Rc::downgrade(self);
            keep.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    glib::idle_add_local_once(move || this.bar.set_visible(false));
                }
            });
            let reload = gtk::Button::with_label("Load disk");
            let this = Rc::downgrade(self);
            reload.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    glib::idle_add_local_once(move || this.load());
                }
            });
            self.bar_actions.append(&keep);
            self.bar_actions.append(&reload);
        }
        self.bar.set_visible(true);
    }

    fn attach_vim(self: &Rc<Self>, weak: &Weak<App>) {
        let vim = sourceview::VimIMContext::new();
        vim.set_client_widget(Some(&self.view));
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.set_im_context(Some(&vim));
        self.view.add_controller(keys.clone());
        let (this, weak) = (Rc::downgrade(self), weak.clone());
        vim.connect_write(move |_, _, _| {
            if let Some(this) = this.upgrade() {
                let weak = weak.clone();
                glib::idle_add_local_once(move || this.save(&weak));
            }
        });
        *self.vim.borrow_mut() = Some((vim, keys));
    }

    fn detach_vim(&self) {
        let taken = self.vim.borrow_mut().take();
        if let Some((vim, keys)) = taken {
            self.view.remove_controller(&keys);
            vim.set_client_widget(None::<&gtk::Widget>);
        }
    }

    pub(crate) fn goto_line(&self, line: u32, column: u32) {
        let Some(mut iter) = self.buffer.iter_at_line(line.saturating_sub(1) as i32) else {
            return;
        };
        let offset = column.saturating_sub(1) as i32;
        if offset > 0 && !iter.ends_line() {
            iter.forward_chars(offset);
            if !iter.ends_line() && iter.line() != line.saturating_sub(1) as i32 {
                iter.backward_char();
            }
        }
        self.buffer.place_cursor(&iter);
        let mark = self.buffer.get_insert();
        let view = self.view.clone();
        glib::idle_add_local_once(move || {
            view.scroll_to_mark(&mark, 0.1, true, 0.0, 0.3);
            view.grab_focus();
        });
    }

    pub(crate) fn restyle(self: &Rc<Self>, app: &App) {
        self.buffer
            .set_style_scheme(crate::theme::scheme(crate::theme::dark()).as_ref());
        let prose_on = self.prose.is_some() && self.mode.get() != MarkdownMode::Source;
        if let Some(prose) = &self.prose {
            prose.set_on(prose_on, app);
        }
        let settings = app.settings.borrow();
        if prose_on {
            self.view.set_wrap_mode(gtk::WrapMode::WordChar);
        } else {
            self.view.set_monospace(true);
            self.view.set_show_line_numbers(settings.show_line_numbers);
            self.view.set_highlight_current_line(false);
            self.buffer.set_highlight_syntax(true);
            self.view
                .set_wrap_mode(if settings.editor_wraps(&self.path) {
                    gtk::WrapMode::WordChar
                } else {
                    gtk::WrapMode::None
                });
        }
        let vim_mode = settings.vim_mode;
        drop(settings);
        if vim_mode && self.vim.borrow().is_none() {
            self.attach_vim(&self.app.clone());
        } else if !vim_mode {
            self.detach_vim();
        }
    }

    fn set_mode(self: &Rc<Self>, mode: MarkdownMode) -> bool {
        if self.prose.is_none() {
            return false;
        }
        if self.mode.replace(mode) != mode {
            let mut prefs = prose_file_prefs(&self.path);
            prefs.mode = mode;
            if let Err(error) = set_prose_file_prefs(&self.path, prefs) {
                eprintln!("[xg] could not remember the Markdown mode: {error}");
            }
            if let Some(app) = self.app.upgrade() {
                self.restyle(&app);
            }
        }
        self.sync_switch();
        true
    }

    pub(crate) fn save(self: &Rc<Self>, app: &Weak<App>) {
        if self.loading.get() {
            return;
        }
        let saver = sourceview::FileSaver::new(&self.buffer, &self.file);
        saver.set_flags(sourceview::FileSaverFlags::IGNORE_MODIFICATION_TIME);
        let (this, app) = (self.clone(), app.clone());
        glib::spawn_future_local(async move {
            let result = saver.save_future(glib::Priority::DEFAULT).0.await;
            let name = this
                .path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            if let Some(app) = app.upgrade() {
                match result {
                    Ok(()) => {
                        this.bar.set_visible(false);
                        app.toast(&format!("Saved {name}"));
                    }
                    Err(error) => {
                        app.toast(&format!("Couldn\u{2019}t save {name}: {}", error.message()))
                    }
                }
            }
        });
    }
}

impl Drop for EditorView {
    fn drop(&mut self) {
        // The Vim input method pops its state off the buffer when it is freed.
        // Detach now, while the view and buffer are still alive.
        self.detach_vim();
        if let Some(monitor) = self.monitor.take() {
            monitor.cancel();
        }
    }
}
