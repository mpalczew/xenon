//! Xenon, GTK frontend: workspaces, panes, and terminals on GTK 4, libadwaita
//! and VTE. It reads and writes the GPUI app's file formats in its own state
//! directory (see `data.rs`). Product rules live in the shared `xenon_*` crates;
//! this crate draws.
//!
//! Usage: `xenon-bin [--screenshot PNG] [open [--pane P] PATH[:LINE[:COL]] | PATH...]`
//!   PATH          a folder becomes a workspace, a file opens in the workspace
//!                 that holds it. A running instance on the same data dir
//!                 (`XENON_DATA_DIR`, one per A/B slot) receives it over `ipc.sock`.
//!   --screenshot  render the window to PNG shortly after start, then quit
//!
//! `GSK_RENDERER` wins when set; otherwise `ngl`, which idles near zero.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};

mod actions;
mod app;
mod clipboard;
mod command_palette;
mod commands;
mod dialogs;
mod diff_text;
mod diff_view;
mod editor;
mod empty;
mod events;
mod files;
mod find;
mod finder;
mod git;
mod git_ops;
mod git_rows;
mod git_section;
mod ipc;
mod keys;
mod layout;
mod links;
mod model;
mod model_describe;
mod model_status;
mod model_tabs;
mod model_tasks;
mod palette;
mod palette_list;
mod pane;
mod pane_drop;
mod persist;
mod picker;
mod prefs;
mod prefs_agents;
mod prefs_apply;
mod prefs_pages;
mod prefs_phone;
mod prefs_state;
mod prose;
mod prose_images;
mod prose_tags;
mod remote;
mod remote_fit;
mod remote_host;
mod remote_mirror;
mod remote_session;
mod remote_ui;
mod screenshot;
mod sidebar;
mod sidebar_ui;
mod tab_ui;
mod tasks;
mod tasks_ui;
mod term;
mod term_menu;
mod theme;
mod window;
mod worklist;
mod worklist_item;
mod worklist_list;
mod workspaces;

use app::App;

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
    // `--screenshot PNG` is ours; everything else is the `xenon open` command line.
    let mut rest: Vec<String> = std::env::args().skip(1).collect();
    let shot = rest
        .iter()
        .position(|a| a == "--screenshot")
        .filter(|i| i + 1 < rest.len())
        .map(|i| {
            let path = rest.remove(i + 1);
            rest.remove(i);
            path
        });
    let command = xenon_store::parse_cli(rest);
    let request = command.to_request();
    let mut listener = None;
    if shot.is_none() {
        if xenon_store::try_handoff_request(&request) {
            return glib::ExitCode::SUCCESS;
        }
        listener = xenon_store::bind_server()
            .inspect_err(|error| eprintln!("[xg] ipc bind failed: {error}"))
            .ok();
        if listener.is_some() {
            let pid = xenon_store::data_dir().join("xenon.pid");
            let _ = std::fs::write(pid, std::process::id().to_string());
        }
    }

    adw::init().expect("GTK init failed");
    // One process per slot: no GApplication uniqueness, the slot's socket routes opens.
    let gtk_app = adw::Application::builder()
        .application_id("dev.xenon.Xenon")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let slot: Rc<RefCell<Option<Rc<App>>>> = Rc::default();
    let keymap_text =
        std::fs::read_to_string(xenon_store::data_dir().join("keymap.json")).unwrap_or_default();
    let user_keys = keys::user_entries(&keymap_text);

    let ensure = {
        let slot = slot.clone();
        move |gtk_app: &adw::Application| -> Rc<App> {
            slot.borrow_mut()
                .get_or_insert_with(|| window::build(gtk_app, &user_keys))
                .clone()
        }
    };
    let listener = RefCell::new(listener);
    let boot = RefCell::new(Some(request));
    gtk_app.connect_activate(move |gtk_app| {
        let app = ensure(gtk_app);
        if let Some(listener) = listener.take() {
            ipc::serve(&app, listener);
        }
        match boot.take() {
            Some(request) => app.handle_request(request),
            None => app.win.present(),
        }
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
    // GTK would parse the command line itself; our arguments are already read.
    gtk_app.run_with_args::<&str>(&[])
}
