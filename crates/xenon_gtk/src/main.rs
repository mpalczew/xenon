//! Xenon GTK spike: proves GTK4 + libadwaita + VTE + GtkSourceView 5 from Rust.
//!
//! Flags (all optional):
//!   FILE...            files for Ctrl+Shift+E (the first one opens at startup with --open)
//!   --tab CMD          open an extra terminal tab and type CMD into it (repeatable)
//!   --select N         select tab N (0-based) after startup
//!   --open             open the first FILE in a source tab at startup
//!   --selftest         run the scripted capability demo, log results, then quit
//!   --screenshot PNG   render the window to PNG (offscreen paintable path), then quit
//!   --search REGEX     set the VTE search regex on every new terminal
//!
//! All demo output goes to stderr with the prefix "[xg]".

// The legacy window-title signal is deprecated in VTE 0.78 but still emitted; the demo
// logs both it and the termprop.
#![allow(deprecated)]
// Spike code: long setup functions are deliberate; the migration crate will split them.
#![allow(clippy::too_many_lines)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};
use vte::prelude::*;

macro_rules! log {
    ($($a:tt)*) => { eprintln!("[xg] {}", format!($($a)*)) };
}

mod selftest;
mod source;
mod term;

#[derive(Default)]
pub(crate) struct Args {
    pub(crate) files: Vec<String>,
    pub(crate) tabs: Vec<String>,
    pub(crate) select: Option<u32>,
    pub(crate) open: bool,
    pub(crate) selftest: bool,
    pub(crate) screenshot: Option<String>,
    pub(crate) search: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--tab" => a.tabs.extend(it.next()),
            "--select" => a.select = it.next().and_then(|v| v.parse().ok()),
            "--open" => a.open = true,
            "--selftest" => a.selftest = true,
            "--screenshot" => a.screenshot = it.next(),
            "--search" => a.search = it.next(),
            _ => a.files.push(s),
        }
    }
    a
}

/// Per-terminal counters for the output-burst demo.
#[derive(Default)]
pub(crate) struct Stats {
    pub(crate) contents_changed: Cell<u64>,
    pub(crate) bells: Cell<u64>,
}

pub(crate) struct Ctx {
    pub(crate) view: adw::TabView,
    pub(crate) args: Args,
    pub(crate) next_id: Cell<u32>,
    pub(crate) win: RefCell<Option<adw::ApplicationWindow>>,
    pub(crate) stats: RefCell<Vec<(adw::TabPage, vte::Terminal, Rc<Stats>)>>,
}

fn main() -> glib::ExitCode {
    // On the NVIDIA Wayland test machine GTK's default Vulkan renderer keeps a driver
    // thread waking ~100x/s while idle (Ptyxis shows the same). The ngl (GL) renderer
    // idles at ~0 wakeups. Default to ngl; GSK_RENDERER in the environment still wins.
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: first statement of main, before any thread exists.
        unsafe { std::env::set_var("GSK_RENDERER", "ngl") };
    }
    adw::init().expect("GTK init failed");
    // Unique application: it owns its name on the session bus, which exports the app.*
    // actions below over org.gtk.Actions so an agent can drive it without synthetic
    // input (Wayland offers no portable key injection). XENON_GTK_APP_ID lets several
    // instances run side by side for measurements.
    let id = std::env::var("XENON_GTK_APP_ID").unwrap_or_else(|_| "dev.xenon.GtkSpike".into());
    let app = adw::Application::builder().application_id(id).build();
    let args = parse_args();
    let ctx = Rc::new(Ctx {
        view: adw::TabView::new(),
        args,
        next_id: Cell::new(0),
        win: RefCell::new(None),
        stats: RefCell::new(Vec::new()),
    });
    app.connect_activate(move |app| build(app, &ctx));
    // Empty args so GTK does not try to parse our own flags.
    app.run_with_args::<&str>(&[])
}

fn build(app: &adw::Application, ctx: &Rc<Ctx>) {
    let tabs = adw::TabBar::new();
    tabs.set_view(Some(&ctx.view));
    tabs.set_autohide(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&tabs);
    ctx.view.set_vexpand(true);
    content.append(&ctx.view);

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Xenon GTK Spike")
        .default_width(1000)
        .default_height(650)
        .content(&content)
        .build();

    // Window title stays fixed so agents can find the window; tab titles carry the shell title.

    // Keyboard: app-level accelerators that fire even while a terminal has focus.
    let new_term = gio::SimpleAction::new("new-terminal", None);
    {
        let ctx = ctx.clone();
        new_term.connect_activate(move |_, _| {
            term::new_terminal(&ctx, None);
        });
    }
    let close_tab = gio::SimpleAction::new("close-tab", None);
    {
        let ctx = ctx.clone();
        close_tab.connect_activate(move |_, _| {
            if let Some(p) = ctx.view.selected_page() {
                ctx.view.close_page(&p);
            }
        });
    }
    let open_file = gio::SimpleAction::new("open-file", None);
    {
        let ctx = ctx.clone();
        open_file.connect_activate(move |_, _| {
            match ctx.args.files.first() {
                Some(f) => source::open_source(&ctx, f),
                None => log!("open-file: no FILE given on the command line"),
            };
        });
    }
    let find_next = gio::SimpleAction::new("find-next", None);
    let find_prev = gio::SimpleAction::new("find-prev", None);
    let dump = gio::SimpleAction::new("dump-screen", None);
    {
        let ctx = ctx.clone();
        find_next.connect_activate(move |_, _| {
            if let Some(t) = term::current_terminal(&ctx) {
                log!("search_find_next -> {}", t.search_find_next());
            }
        });
    }
    {
        let ctx = ctx.clone();
        find_prev.connect_activate(move |_, _| {
            if let Some(t) = term::current_terminal(&ctx) {
                log!("search_find_previous -> {}", t.search_find_previous());
            }
        });
    }
    {
        let ctx = ctx.clone();
        dump.connect_activate(move |_, _| {
            if let Some(t) = term::current_terminal(&ctx) {
                term::dump_screen(&t);
            }
        });
    }
    // Agent drive path (D-Bus): app.feed-child "text", app.screenshot "/path.png".
    let feed = gio::SimpleAction::new("feed-child", Some(glib::VariantTy::STRING));
    {
        let ctx = ctx.clone();
        feed.connect_activate(move |_, p| {
            if let (Some(t), Some(text)) = (term::current_terminal(&ctx), p.and_then(|p| p.str())) {
                t.feed_child(text.as_bytes());
            }
        });
    }
    let shot = gio::SimpleAction::new("screenshot", Some(glib::VariantTy::STRING));
    {
        let win = win.clone();
        shot.connect_activate(move |_, p| {
            if let Some(path) = p.and_then(|p| p.str()) {
                log!("screenshot {path}: {:?}", selftest::render_png(&win, path));
            }
        });
    }
    for a in [
        &feed, &shot, &new_term, &close_tab, &open_file, &find_next, &find_prev, &dump,
    ] {
        app.add_action(a);
    }
    app.set_accels_for_action("app.new-terminal", &["<Control><Shift>t"]);
    app.set_accels_for_action("app.close-tab", &["<Control><Shift>w"]);
    app.set_accels_for_action("app.open-file", &["<Control><Shift>e"]);
    app.set_accels_for_action("app.find-next", &["<Control><Shift>g"]);
    app.set_accels_for_action("app.find-prev", &["<Control><Shift>h"]);
    app.set_accels_for_action("app.dump-screen", &["<Control><Shift>d"]);

    *ctx.win.borrow_mut() = Some(win.clone());
    // Close the app when the last tab goes away.
    {
        let app = app.clone();
        ctx.view.connect_close_page(move |v, page| {
            v.close_page_finish(page, true);
            if v.n_pages() == 0 {
                app.quit();
            }
            glib::Propagation::Stop
        });
    }

    term::new_terminal(ctx, None);
    for cmd in ctx.args.tabs.clone() {
        term::new_terminal(ctx, Some(cmd));
    }
    if ctx.args.open
        && let Some(f) = ctx.args.files.first()
    {
        source::open_source(ctx, f);
    }
    if let Some(n) = ctx.args.select
        && let Some(p) = ctx.view.nth_page(n as i32).into()
    {
        ctx.view.set_selected_page(&p);
    }
    win.present();

    if ctx.args.selftest || ctx.args.screenshot.is_some() {
        let ctx = ctx.clone();
        let win = win.clone();
        let app = app.clone();
        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            selftest::selftest_phase1(&ctx, &win, &app);
        });
    }
}
