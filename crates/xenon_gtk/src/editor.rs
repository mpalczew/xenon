//! A file tab: GtkSourceView with syntax highlighting and, when the settings
//! ask for it, the Vim input method. Ctrl+S and `:w` write the file. This is
//! the stand-in editor until the GTK editor exists; it is a plain text view
//! with no LSP, diff gutter or prose mode.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use sourceview::prelude::*;
use xenon_core::{TabId, WorkspaceId};

use crate::app::App;

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
    app: Weak<App>,
}

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
    String::from_utf8(bytes).map_err(|_| "not UTF-8 text".into())
}

impl EditorView {
    pub(crate) fn new(app: &Rc<App>, ws: WorkspaceId, tab: TabId, path: &Path) -> Rc<Self> {
        let text = load(path).unwrap_or_else(|why| format!("Cannot show this file: {why}\n"));
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
        scroller.add_css_class("xeditor-scroll");
        let editor = Rc::new(Self {
            root: scroller.upcast(),
            view,
            buffer,
            path: path.to_path_buf(),
            loading: Cell::new(false),
            vim: RefCell::new(None),
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
            if state.contains(gdk::ModifierType::CONTROL_MASK)
                && matches!(key, gdk::Key::s | gdk::Key::S)
                && let Some(this) = this.upgrade()
            {
                this.save(&weak);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        editor.view.add_controller(keys);
        editor.buffer.set_modified(false);
        editor
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

    /// Font, wrap and color scheme from the settings and theme.
    pub(crate) fn restyle(self: &Rc<Self>, app: &App) {
        let dark = adw::StyleManager::default().is_dark();
        let name = if dark { "Adwaita-dark" } else { "Adwaita" };
        if let Some(scheme) = sourceview::StyleSchemeManager::default().scheme(name) {
            self.buffer.set_style_scheme(Some(&scheme));
        }
        let settings = app.settings.borrow();
        self.view.set_show_line_numbers(settings.show_line_numbers);
        self.view
            .set_wrap_mode(if settings.editor_wraps(&self.path) {
                gtk::WrapMode::WordChar
            } else {
                gtk::WrapMode::None
            });
        let vim_mode = settings.vim_mode;
        drop(settings);
        if vim_mode && self.vim.borrow().is_none() {
            self.attach_vim(&self.app.clone());
        } else if !vim_mode {
            self.detach_vim();
        }
    }

    pub(crate) fn save(&self, app: &Weak<App>) {
        if self.loading.get() {
            return;
        }
        let (a, b) = self.buffer.bounds();
        let text = self.buffer.text(&a, &b, false);
        let result = std::fs::write(&self.path, text.as_str());
        let name = self
            .path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if let Some(app) = app.upgrade() {
            match result {
                Ok(()) => {
                    self.buffer.set_modified(false);
                    app.toast(&format!("Saved {name}"));
                }
                Err(error) => app.toast(&format!("Could not save {name}: {error}")),
            }
        }
    }
}
