//! A file tab: GtkSourceView with syntax highlighting and, when the settings
//! ask for it, the Vim input method. Ctrl+S and `:w` write the file. This is
//! the stand-in editor until the GTK editor exists; it is a plain text view
//! with no LSP or diff gutter. Markdown files open in prose mode (`prose.rs`);
//! Alt+1 and Alt+3 switch between Prose and Source.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use sourceview::prelude::*;
use xenon_core::{TabId, WorkspaceId};
use xenon_settings::{MarkdownMode, prose_file_prefs, set_prose_file_prefs};

use crate::app::App;
use crate::prose::{self, Prose};

/// Files above this size open in the default app instead.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) struct EditorView {
    pub root: gtk::Widget,
    pub view: sourceview::View,
    pub buffer: sourceview::Buffer,
    pub path: PathBuf,
    /// True while we load text, so loading does not count as an edit.
    loading: Cell<bool>,
    /// The Vim input method and its key controller while Vim mode is on.
    vim: RefCell<Option<(sourceview::VimIMContext, gtk::EventControllerKey)>>,
    /// Markdown writing mode, for Markdown files.
    pub(crate) prose: Option<Rc<Prose>>,
    mode: Cell<MarkdownMode>,
    app: Weak<App>,
    scroller: gtk::ScrolledWindow,
    /// "File changed on disk" bar: message and its two buttons.
    bar: gtk::Box,
    bar_text: gtk::Label,
    bar_actions: gtk::Box,
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// True from the first disk event of a burst until it is handled.
    sync_pending: Cell<bool>,
}

/// Wait this long after a disk event, so an agent's several writes read once.
const DISK_SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

/// Why a file cannot open here, if it cannot (cheap: no read).
pub(crate) fn check(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".into());
    }
    if meta.len() > MAX_BYTES {
        return Err("larger than 4 MB".into());
    }
    Ok(())
}

/// Why a file cannot open here, or the text to show.
pub(crate) fn load(path: &Path) -> Result<String, String> {
    check(path)?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "xenon cannot edit this binary file".into())
}

impl EditorView {
    pub(crate) fn new(app: &Rc<App>, ws: WorkspaceId, tab: TabId, path: &Path) -> Rc<Self> {
        let text = load(path).unwrap_or_else(|why| format!("{why}\n"));
        let buffer = sourceview::Buffer::new(None);
        buffer.set_text(&text);
        buffer.set_highlight_syntax(true);
        if let Some(lang) =
            sourceview::LanguageManager::default().guess_language(Some(path), None::<&str>)
        {
            buffer.set_language(Some(&lang));
        }
        let view = sourceview::View::with_buffer(&buffer);
        view.add_css_class("xeditor");
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
        let bar_text = gtk::Label::new(None);
        bar_text.set_hexpand(true);
        bar_text.set_xalign(0.0);
        bar_text.set_wrap(true);
        let bar_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("toolbar");
        bar.append(&bar_text);
        bar.append(&bar_actions);
        bar.set_visible(false);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&bar);
        root.append(&scroller);
        let editor = Rc::new(Self {
            root: root.upcast(),
            scroller,
            bar,
            bar_text,
            bar_actions,
            monitor: RefCell::new(None),
            sync_pending: Cell::new(false),
            view,
            buffer,
            path: path.to_path_buf(),
            loading: Cell::new(false),
            vim: RefCell::new(None),
            prose,
            mode: Cell::new(mode),
            app: Rc::downgrade(app),
        });
        editor.restyle(app);
        let weak: Weak<App> = Rc::downgrade(app);
        {
            let weak = weak.clone();
            let this = Rc::downgrade(&editor);
            editor.buffer.connect_modified_changed(move |b| {
                if let (Some(app), Some(_)) = (weak.upgrade(), this.upgrade()) {
                    app.editor_modified(ws, tab, b.is_modified());
                }
            });
        }
        // Ctrl+S saves. Capture phase, so it wins over the Vim input method.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = Rc::downgrade(&editor);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = this.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = state.contains(gdk::ModifierType::ALT_MASK);
            if ctrl && matches!(key, gdk::Key::s | gdk::Key::S) {
                this.save(&weak);
                return glib::Propagation::Stop;
            }
            let handled = match key {
                gdk::Key::_1 if alt => this.set_mode(MarkdownMode::Prose),
                gdk::Key::_3 if alt => this.set_mode(MarkdownMode::Source),
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => this
                    .prose
                    .as_ref()
                    .is_some_and(|p| p.on.get() && p.toggle_task()),
                _ => false,
            };
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        editor.view.add_controller(keys);
        editor.buffer.set_modified(false);
        editor.watch_disk();
        editor
    }

    /// Watch the file; every event schedules one settled look at the disk.
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
                }
            });
        });
        *self.monitor.borrow_mut() = Some(monitor);
    }

    /// Show the disk text in a clean buffer, or say why it cannot. A dirty
    /// buffer is never overwritten; a deleted file leaves the buffer open
    /// and unsaved.
    fn sync_from_disk(self: &Rc<Self>) {
        if !self.path.exists() {
            self.show_bar(
                "File was deleted on disk. Your buffer is still open.",
                false,
            );
            self.buffer.set_modified(true);
            return;
        }
        let Ok(text) = load(&self.path) else { return };
        let (a, b) = self.buffer.bounds();
        if self.buffer.text(&a, &b, true) == text {
            self.bar.set_visible(false);
        } else if self.buffer.is_modified() {
            self.show_bar(
                "File changed on disk. Keep your edits, or reload from disk?",
                true,
            );
        } else {
            self.replace_text(&text);
        }
    }

    /// Swap in new text, keeping the cursor line and the scroll position.
    fn replace_text(&self, text: &str) {
        let cursor = self.buffer.iter_at_offset(self.buffer.cursor_position());
        let (line, col) = (cursor.line(), cursor.line_offset());
        let scroll = self.scroller.vadjustment().value();
        self.loading.set(true);
        // No irreversible action: Vim insert mode keeps a user action open
        // on the buffer, and GTK refuses to begin one inside it.
        self.buffer.set_text(text);
        self.buffer.set_modified(false);
        self.loading.set(false);
        let at = self
            .buffer
            .iter_at_line_offset(line, col)
            .unwrap_or_else(|| self.buffer.end_iter());
        self.buffer.place_cursor(&at);
        self.bar.set_visible(false);
        let adjustment = self.scroller.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(scroll));
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
                    this.bar.set_visible(false);
                }
            });
            let reload = gtk::Button::with_label("Load disk");
            let this = Rc::downgrade(self);
            reload.connect_clicked(move |_| {
                if let Some(this) = this.upgrade()
                    && let Ok(text) = load(&this.path)
                {
                    this.replace_text(&text);
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
                this.save(&weak);
            }
        });
        *self.vim.borrow_mut() = Some((vim, keys));
    }

    fn detach_vim(&self) {
        if let Some((vim, keys)) = self.vim.borrow_mut().take() {
            self.view.remove_controller(&keys);
            vim.set_client_widget(None::<&gtk::Widget>);
        }
    }

    /// Put the cursor on 1-based `line` and bring it into view.
    pub(crate) fn goto_line(&self, line: u32) {
        let Some(iter) = self.buffer.iter_at_line(line.saturating_sub(1) as i32) else {
            return;
        };
        self.buffer.place_cursor(&iter);
        let mark = self.buffer.create_mark(None, &iter, true);
        let view = self.view.clone();
        glib::idle_add_local_once(move || {
            view.scroll_to_mark(&mark, 0.1, true, 0.0, 0.3);
            view.grab_focus();
        });
    }

    /// Font, wrap and color scheme from the settings and theme.
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

    /// Prose or Source for a Markdown file; remembered per file like the GPUI app.
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
        true
    }

    pub(crate) fn save(&self, app: &Weak<App>) {
        if self.loading.get() {
            return;
        }
        let (a, b) = self.buffer.bounds();
        // Hidden markup is still part of the file.
        let text = self.buffer.text(&a, &b, true);
        let result = std::fs::write(&self.path, text.as_str());
        let name = self
            .path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if let Some(app) = app.upgrade() {
            match result {
                Ok(()) => {
                    self.buffer.set_modified(false);
                    self.bar.set_visible(false);
                    app.toast(&format!("Saved {name}"));
                }
                Err(error) => app.toast(&format!("Couldn\u{2019}t save {name}: {error}")),
            }
        }
    }
}

impl Drop for EditorView {
    fn drop(&mut self) {
        // Let the Vim input method end its open user action while the view
        // and buffer are still alive; otherwise GTK ends it on a freed buffer.
        self.detach_vim();
        if let Some(monitor) = self.monitor.take() {
            monitor.cancel();
        }
    }
}
