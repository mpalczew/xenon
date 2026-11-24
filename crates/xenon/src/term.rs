use std::cell::Cell;
use std::path::Path;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use vte::prelude::*;
use xenon_core::{TabId, TermId, WorkspaceId};
use xenon_term::{Size, Terminals};

use crate::app::{App, defer};
use crate::find::FindBar;

pub(crate) struct TermView {
    pub root: gtk::Box,
    pub term: vte::Terminal,
    pub find: FindBar,
    pub id: TermId,
    pub cwd: std::path::PathBuf,
    pub pid: Cell<Option<i32>>,
    attached: Cell<bool>,
    pinned: Cell<bool>,
    claimed: Cell<Option<(i64, i64)>>,
    /// A replay turned focus reporting on; VTE answers with a focus report to drop.
    skip_focus: Cell<bool>,
}

fn parse(color: &str) -> gdk::RGBA {
    gdk::RGBA::parse(color).unwrap_or(gdk::RGBA::BLACK)
}

impl TermView {
    pub(crate) fn new(
        app: &Rc<App>,
        ws: WorkspaceId,
        tab: TabId,
        id: TermId,
        cwd: &Path,
    ) -> Rc<Self> {
        let term = vte::Terminal::new();
        term.set_hexpand(true);
        term.set_vexpand(true);
        term.set_scrollback_lines(
            xenon_store::load_service_settings()
                .unwrap_or_default()
                .terminal_scrollback_lines as i64,
        );
        term.set_allow_hyperlink(true);
        term.set_audible_bell(false);
        term.set_scroll_on_output(false);
        term.set_scroll_unit_is_pixels(true);
        term.set_enable_fallback_scrolling(false);
        term.set_scroll_on_keystroke(true);
        term.set_cursor_blink_mode(vte::CursorBlinkMode::Off);
        let find = FindBar::new(&term);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&find.revealer);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroller.set_vexpand(true);
        scroller.set_child(Some(&term));
        root.append(&scroller);
        let view = Rc::new(Self {
            root,
            term,
            find,
            id,
            cwd: cwd.to_path_buf(),
            pid: Cell::new(None),
            attached: Cell::new(false),
            pinned: Cell::new(false),
            claimed: Cell::new(None),
            skip_focus: Cell::new(false),
        });
        crate::links::install(app, &view, ws, tab);
        crate::term_menu::install(&view);
        connect_signals(app, &view);
        view
    }

    pub(crate) fn restyle(&self, family: &str, size_px: f32, dark: bool) {
        let mut font = gtk::pango::FontDescription::new();
        font.set_family(family);
        font.set_absolute_size(f64::from(size_px) * f64::from(gtk::pango::SCALE));
        self.term.set_font_desc(Some(&font));
        let (fg, bg, ansi) = xenon_core::terminal_palette(dark);
        let ansi: Vec<gdk::RGBA> = ansi.iter().map(|c| parse(c)).collect();
        let refs: Vec<&gdk::RGBA> = ansi.iter().collect();
        self.term
            .set_colors(Some(&parse(fg)), Some(&parse(bg)), &refs);
    }

    pub(crate) fn live_cwd(&self) -> std::path::PathBuf {
        self.pid
            .get()
            .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/cwd")).ok())
            .unwrap_or_else(|| self.cwd.clone())
    }

    fn grid(&self) -> Option<Size> {
        let cols = u16::try_from(self.term.column_count()).ok()?;
        let rows = u16::try_from(self.term.row_count()).ok()?;
        Size::new(cols, rows)
    }

    pub(crate) fn attach(&self, holders: &Terminals) {
        if self.attached.replace(true) {
            return;
        }
        if let Some(size) = self.grid() {
            self.claimed
                .set(Some((i64::from(size.cols()), i64::from(size.rows()))));
            holders.attach(self.id, size);
        }
    }

    pub(crate) fn report_size(&self, holders: &Terminals) {
        if !self.attached.get() || self.pinned.get() {
            return;
        }
        let now = (self.term.column_count(), self.term.row_count());
        if self.claimed.get() == Some(now) {
            return;
        }
        if let Some(size) = self.grid() {
            self.claimed.set(Some(now));
            holders.claim(self.id, size);
        }
    }

    pub(crate) fn follow(&self, size: Size) {
        let (cols, rows) = (i64::from(size.cols()), i64::from(size.rows()));
        self.term.set_size(cols, rows);
        let held = self.claimed.get().is_none_or(|own| own != (cols, rows));
        self.pinned.set(held);
        self.term.set_hexpand(!held);
        self.term.set_vexpand(!held);
        let align = if held {
            gtk::Align::Start
        } else {
            gtk::Align::Fill
        };
        self.term.set_halign(align);
        self.term.set_valign(align);
    }

    pub(crate) fn feed(&self, bytes: &[u8]) {
        if bytes.windows(8).any(|w| w == b"\x1b[?1004h") {
            self.skip_focus.set(true);
        }
        self.term.feed(bytes);
    }

    fn is_replay_focus_report(&self, text: &str) -> bool {
        matches!(text, "\x1b[I" | "\x1b[O") && self.skip_focus.replace(false)
    }
}

pub(crate) fn child_env() -> Vec<String> {
    let defaulted_renderer = std::env::var_os("XENON_GTK_GSK_DEFAULTED").is_some();
    std::env::vars()
        .filter(|(k, _)| k != "XENON_GTK_GSK_DEFAULTED")
        .filter(|(k, _)| !(defaulted_renderer && k == "GSK_RENDERER"))
        .map(|(k, v)| format!("{k}={v}"))
        .collect()
}

/// Bytes a key sends that VTE would send differently. Shift+Enter is a line
/// feed (like Ctrl+J), as the old terminal sent it: Claude Code and Codex read
/// it as "new line", while VTE's carriage return would submit.
pub(crate) fn key_bytes(key: gdk::Key, state: gdk::ModifierType) -> Option<&'static [u8]> {
    let plain = state & gtk::accelerator_get_default_mod_mask();
    (plain == gdk::ModifierType::SHIFT_MASK && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter))
        .then_some(b"\n")
}

fn connect_signals(app: &Rc<App>, view: &Rc<TermView>) {
    let weak: Weak<App> = Rc::downgrade(app);
    let term = &view.term;
    {
        let (weak, view) = (weak.clone(), Rc::downgrade(view));
        term.connect_contents_changed(move |_| {
            let view = view.clone();
            defer(&weak, move |app| {
                if let Some(view) = view.upgrade() {
                    view.report_size(&app.holders);
                }
            });
        });
    }
    {
        let (weak, view) = (weak.clone(), Rc::downgrade(view));
        term.connect_map(move |_| {
            let view = view.clone();
            defer(&weak, move |app| {
                if let Some(view) = view.upgrade() {
                    view.attach(&app.holders);
                }
            });
        });
    }
    {
        let (weak, view) = (weak.clone(), Rc::downgrade(view));
        // sync: answer or ordering before return: keystrokes, pastes and VTE's replies must reach the service in the order they were typed.
        term.connect_commit(move |_, text, _| {
            let (Some(app), Some(view)) = (weak.upgrade(), view.upgrade()) else {
                return;
            };
            if !view.is_replay_focus_report(text) {
                app.holders.input(view.id, text.as_bytes());
            }
        });
    }
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let term = view.term.downgrade();
        // sync: answer or ordering before return: keystrokes must reach the shell in order and the key must be consumed now.
        keys.connect_key_pressed(move |_, key, _, state| {
            if let Some(bytes) = key_bytes(key, state)
                && let Some(term) = term.upgrade()
            {
                term.feed_child(bytes);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    term.add_controller(keys);
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    {
        let term = view.term.downgrade();
        // sync: answer or ordering before return: must say at once whether the drop was taken; it only types the paths.
        drop.connect_drop(move |_, value, _, _| {
            let Ok(files) = value.get::<gdk::FileList>() else {
                return false;
            };
            let paths: Vec<_> = files.files().iter().filter_map(|f| f.path()).collect();
            let (false, Some(term)) = (paths.is_empty(), term.upgrade()) else {
                return false;
            };
            term.paste_text(&xenon_core::terminal_paths_text(paths));
            true
        });
    }
    term.add_controller(drop);
}
