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
use crate::theme::Palette;

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
            last_output: Cell::new(Instant::now()),
            quiet_armed: Cell::new(false),
        });
        crate::links::install(app, &view, ws, tab);
        connect_signals(app, &view, ws, tab);
        view
    }

    /// Font and colors from the current settings and theme.
    pub(crate) fn restyle(&self, family: &str, size_px: f32, palette: &Palette) {
        let mut font = gtk::pango::FontDescription::new();
        font.set_family(family);
        font.set_absolute_size(f64::from(size_px) * f64::from(gtk::pango::SCALE));
        self.term.set_font_desc(Some(&font));
        let (fg, bg, ansi) = palette.terminal();
        let ansi: Vec<gdk::RGBA> = ansi.iter().map(|c| parse(c)).collect();
        let refs: Vec<&gdk::RGBA> = ansi.iter().collect();
        self.term
            .set_colors(Some(&parse(&fg)), Some(&parse(&bg)), &refs);
    }

    pub(crate) fn spawn(self: &Rc<Self>, cwd: &std::path::Path) {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let shell_name = shell.clone();
        let env: Vec<String> = child_env();
        let env_refs: Vec<&str> = env.iter().map(String::as_str).collect();
        let this = self.clone();
        self.term.spawn_async(
            vte::PtyFlags::DEFAULT,
            Some(&cwd.to_string_lossy()),
            &[&shell],
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
/// program started here (`xenon open`, another GTK app) does not inherit it.
fn child_env() -> Vec<String> {
    const DROP: [&str; 6] = [
        "TERM",
        "COLORTERM",
        "XENON_DATA_DIR",
        "XENON_SLOT",
        "XENON_GTK_APP_ID",
        "XENON_GTK_GSK_DEFAULTED",
    ];
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
        term.connect_child_exited(move |_, status| {
            if let Some(app) = weak.upgrade() {
                app.term_exited(ws, tab, (status >> 8) & 0xff);
            }
        });
    }
    // Typing is interaction: capture phase, so the key is seen even when an
    // app accelerator or the find bar handles it first.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let weak = weak.clone();
        keys.connect_key_pressed(move |_, _, _, _| {
            if let Some(app) = weak.upgrade() {
                app.term_interaction(ws, tab);
            }
            glib::Propagation::Proceed
        });
    }
    term.add_controller(keys);
    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter(move |_| {
        if let Some(app) = weak.upgrade() {
            app.term_focused(ws, tab);
        }
    });
    term.add_controller(focus);
}
