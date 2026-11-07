//! Xenon, GTK frontend: workspaces, panes, and terminals on GTK 4, libadwaita
//! and VTE. It reads and writes the GPUI app's file formats in its own state
//! directory (see `data.rs`). Product rules live in the shared `xenon_*` crates;
//! this crate draws.
//!
//! Usage: `xenon-gtk [--reseed] [--screenshot PNG] [DIR...]`
//!   DIR           open each folder as a workspace (a running instance with the
//!                 same app id receives it instead of starting a second window)
//!   --reseed      move ~/.xenon-gtk aside and seed again from the GPUI app
//!   --screenshot  render the window to PNG shortly after start, then quit
//!
//! Environment: `XENON_GTK_APP_ID` (default `dev.xenon.Gtk`) and
//! `XENON_GTK_DATA_DIR` (default `~/.xenon-gtk`) let several instances run side
//! by side. `GSK_RENDERER` wins when set; otherwise `ngl`, which idles near zero.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};

mod actions;
mod app;
mod commands;
mod data;
mod dialogs;
mod empty;
mod events;
mod find;
mod keys;
mod layout;
mod links;
mod model;
mod model_describe;
mod model_status;
mod model_tabs;
mod pane;
mod persist;
mod screenshot;
mod sidebar;
mod term;
mod theme;
mod window;
mod workspaces;

use app::App;

#[derive(Default)]
struct Args {
    reseed: bool,
    screenshot: Option<String>,
    dirs: Vec<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--reseed" => args.reseed = true,
            "--screenshot" => args.screenshot = it.next(),
            _ => args.dirs.push(PathBuf::from(arg)),
        }
    }
    args
}

fn main() -> glib::ExitCode {
    // SAFETY: first statements of main, before any thread exists.
    unsafe {
        let broadway = std::env::var("GDK_BACKEND").is_ok_and(|b| b.starts_with("broadway"));
        if std::env::var_os("GSK_RENDERER").is_none() && !broadway {
            // GTK's Vulkan renderer keeps an NVIDIA driver thread waking ~100x/s
            // while idle; ngl idles near zero. Remember that we chose it so
            // shells do not inherit it.
            let name = if gtk::minor_version() >= 20 {
                "gl"
            } else {
                "ngl"
            };
            std::env::set_var("GSK_RENDERER", name);
            std::env::set_var("XENON_GTK_GSK_DEFAULTED", "1");
        }
    }
    let args = parse_args();
    let dir = data::state_dir();
    eprintln!("[xg] {}", data::prepare(&dir, args.reseed));
    // SAFETY: still single-threaded; xenon_store reads its data dir from here.
    unsafe { std::env::set_var("XENON_DATA_DIR", &dir) };

    adw::init().expect("GTK init failed");
    let id = std::env::var("XENON_GTK_APP_ID").unwrap_or_else(|_| "dev.xenon.Gtk".into());
    let gtk_app = adw::Application::builder()
        .application_id(id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let slot: Rc<RefCell<Option<Rc<App>>>> = Rc::default();
    let keymap_text = std::fs::read_to_string(dir.join("keymap.json")).unwrap_or_default();
    let user_keys = keys::user_entries(&keymap_text);

    let ensure = {
        let slot = slot.clone();
        move |gtk_app: &adw::Application| -> Rc<App> {
            slot.borrow_mut()
                .get_or_insert_with(|| window::build(gtk_app, &user_keys))
                .clone()
        }
    };
    let shot = args.screenshot.clone();
    {
        let ensure = ensure.clone();
        gtk_app.connect_activate(move |gtk_app| {
            let app = ensure(gtk_app);
            app.win.present();
            if let Some(path) = shot.clone() {
                glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                    let quit = app.clone();
                    let name = path.clone();
                    screenshot::render_png(&app.win, &path, move |result| {
                        if let Err(error) = result {
                            eprintln!("[xg] screenshot {name}: {error}");
                        }
                        quit.quit();
                    });
                });
            }
        });
    }
    {
        let ensure = ensure.clone();
        gtk_app.connect_open(move |gtk_app, files, _| {
            let app = ensure(gtk_app);
            for file in files {
                if let Some(path) = file.path() {
                    let dir = if path.is_dir() {
                        path
                    } else {
                        path.parent().map(PathBuf::from).unwrap_or_default()
                    };
                    app.open_workspace(&dir);
                }
            }
            app.win.present();
        });
    }
    // GTK parses its own options from argv, so pass only the folders: with
    // files given, GApplication calls `open` here or in the running instance.
    let mut argv = vec![std::env::args().next().unwrap_or_default()];
    argv.extend(args.dirs.iter().map(|d| d.display().to_string()));
    gtk_app.run_with_args(&argv)
}
