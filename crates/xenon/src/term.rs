//! One terminal tab body: a VTE widget in a box with its find bar. It reports
//! raw signals to the `App`, which asks the model what they mean.

use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::Instant;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use vte::prelude::*;
use xenon_core::{TabId, WorkspaceId};

use crate::app::App;
use crate::find::FindBar;

pub(crate) struct TermView {
    /// What goes into the pane's stack: find bar above the terminal.
    pub root: gtk::Box,
    pub term: vte::Terminal,
    pub find: FindBar,
    /// Name of the directory the shell started in (title cleanup).
    pub spawn_dir: String,
    /// Directory the shell started in, for relative link targets.
    pub cwd: std::path::PathBuf,
    pub pid: Cell<Option<i32>>,
    /// The wait status of the child once it ended (a task reads how it ended).
    pub status: Cell<Option<i32>>,
    pub last_output: Cell<Instant>,
    pub quiet_armed: Cell<bool>,
}

fn parse(color: &str) -> gdk::RGBA {
    gdk::RGBA::parse(color).unwrap_or(gdk::RGBA::BLACK)
}

impl TermView {
    pub(crate) fn new(
        app: &Rc<App>,
        ws: WorkspaceId,
        tab: TabId,
        cwd: &std::path::Path,
    ) -> Rc<Self> {
        let term = vte::Terminal::new();
        term.set_hexpand(true);
        term.set_vexpand(true);
        term.set_scrollback_lines(10_000);
        term.set_allow_hyperlink(true);
        term.set_audible_bell(false);
        term.set_scroll_on_output(false);
        term.set_scroll_unit_is_pixels(true);
        term.set_scroll_on_keystroke(true);
        term.set_cursor_blink_mode(vte::CursorBlinkMode::Off);
        let find = FindBar::new(&term);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&find.revealer);
        root.append(&term);
        let spawn_dir = cwd
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let view = Rc::new(Self {
            root,
            term,
            find,
            spawn_dir,
            cwd: cwd.to_path_buf(),
            pid: Cell::new(None),
            status: Cell::new(None),
            last_output: Cell::new(Instant::now()),
            quiet_armed: Cell::new(false),
        });
        crate::links::install(app, &view, ws, tab);
        crate::term_menu::install(&view);
        connect_signals(app, &view, ws, tab);
        view
    }

    /// Font and colors from the current settings and theme.
    pub(crate) fn restyle(&self, family: &str, size_px: f32, dark: bool) {
        let mut font = gtk::pango::FontDescription::new();
        font.set_family(family);
        font.set_absolute_size(f64::from(size_px) * f64::from(gtk::pango::SCALE));
        self.term.set_font_desc(Some(&font));
        let (fg, bg, ansi) = crate::theme::terminal(dark);
        let ansi: Vec<gdk::RGBA> = ansi.iter().map(|c| parse(c)).collect();
        let refs: Vec<&gdk::RGBA> = ansi.iter().collect();
        self.term
            .set_colors(Some(&parse(fg)), Some(&parse(bg)), &refs);
    }

    pub(crate) fn spawn(self: &Rc<Self>, cwd: &std::path::Path) {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        self.spawn_command(cwd, &[shell], &[]);
    }

    /// The shell's current directory, else the one it started in.
    pub(crate) fn live_cwd(&self) -> std::path::PathBuf {
        self.pid
            .get()
            .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/cwd")).ok())
            .unwrap_or_else(|| self.cwd.clone())
    }

    /// Scrollback and screen as text, for a background task's readiness.
    pub(crate) fn all_text(&self) -> String {
        let last = self.term.row_count() - 1;
        let (all, _) = self
            .term
            .text_range_format(vte::Format::Text, -1_000_000, 0, last, -1);
        all.map_or_else(
            || {
                self.term
                    .text_format(vte::Format::Text)
                    .unwrap_or_default()
                    .to_string()
            },
            |text| text.to_string(),
        )
    }

    /// Start `argv` (a task) instead of the shell; `extra_env` adds to ours.
    pub(crate) fn spawn_command(
        self: &Rc<Self>,
        cwd: &std::path::Path,
        argv: &[String],
        extra_env: &[String],
    ) {
        let shell_name = argv.first().cloned().unwrap_or_default();
        let mut env: Vec<String> = child_env();
        env.extend(extra_env.iter().cloned());
        let env_refs: Vec<&str> = env.iter().map(String::as_str).collect();
        let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let this = self.clone();
        self.term.spawn_async(
            vte::PtyFlags::DEFAULT,
            Some(&cwd.to_string_lossy()),
            &argv_refs,
            &env_refs,
            glib::SpawnFlags::SEARCH_PATH,
            || {},
            -1,
            None::<&gio::Cancellable>,
            move |result| match result {
                Ok(pid) => this.pid.set(Some(pid.0)),
                Err(error) => {
                    this.term
                        .feed(format!("could not start {shell_name}: {error}\r\n").as_bytes());
                }
            },
        );
    }
}

/// The environment shells see: ours, minus what only this frontend uses, so a
/// program started here does not inherit it. `XENON_DATA_DIR` and `XENON_SLOT` stay, so
/// `xenon open` in a terminal reaches the slot that runs it.
fn child_env() -> Vec<String> {
    const DROP: [&str; 3] = ["TERM", "COLORTERM", "XENON_GTK_GSK_DEFAULTED"];
    let defaulted_renderer = std::env::var_os("XENON_GTK_GSK_DEFAULTED").is_some();
    let mut env: Vec<String> = std::env::vars()
        .filter(|(k, _)| !DROP.contains(&k.as_str()))
        .filter(|(k, _)| !(defaulted_renderer && k == "GSK_RENDERER"))
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    env.push("TERM=xterm-256color".into());
    env.push("COLORTERM=truecolor".into());
    env
}

/// Bytes a key sends that VTE would send differently. Shift+Enter is a line
/// feed (like Ctrl+J), as the old terminal sent it: Claude Code and Codex read
/// it as "new line", while VTE's carriage return would submit.
pub(crate) fn key_bytes(key: gdk::Key, state: gdk::ModifierType) -> Option<&'static [u8]> {
    let plain = state & gtk::accelerator_get_default_mod_mask();
    (plain == gdk::ModifierType::SHIFT_MASK && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter))
        .then_some(b"\n")
}

fn connect_signals(app: &Rc<App>, view: &Rc<TermView>, ws: WorkspaceId, tab: TabId) {
    let weak: Weak<App> = Rc::downgrade(app);
    let term = &view.term;
    {
        let weak = weak.clone();
        term.connect_contents_changed(move |_| {
            if let Some(app) = weak.upgrade() {
                app.term_output(ws, tab);
            }
        });
    }
    {
        let weak = weak.clone();
        term.connect_bell(move |_| {
            if let Some(app) = weak.upgrade() {
                app.term_bell(ws, tab);
            }
        });
    }
    {
        let weak = weak.clone();
        term.connect_termprop_changed(Some("xterm.title"), move |t, name| {
            let (title, _) = t.termprop_string(name);
            if let Some(app) = weak.upgrade() {
                app.term_title(ws, tab, title.as_deref().unwrap_or(""));
            }
        });
    }
    {
        let weak = weak.clone();
        let view = Rc::downgrade(view);
        term.connect_child_exited(move |_, status| {
            if let Some(view) = view.upgrade() {
                view.status.set(Some(status));
            }
            // Deferred: the exit syncs the panes and may close the tab.
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(app) = weak.upgrade() {
                    app.term_exited(ws, tab, (status >> 8) & 0xff);
                }
            });
        });
    }
    // Typing is interaction: capture phase, so the key is seen even when an
    // app accelerator or the find bar handles it first.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let weak = weak.clone();
        let term = view.term.downgrade();
        keys.connect_key_pressed(move |_, key, _, state| {
            if let Some(app) = weak.upgrade() {
                app.term_interaction(ws, tab);
            }
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
    // Files dropped from a file manager type their quoted paths.
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    {
        let term = view.term.downgrade();
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
