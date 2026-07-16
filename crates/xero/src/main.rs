use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, OsAction, WindowBounds, WindowOptions,
    actions, px, size,
};
use gpui_platform::application;
use xero_store::{IpcRequest, bind_server, parse_cli_paths, serve_forever, try_handoff};
use xero_ui::{
    AddWorkspace, CloseEditor, CloseWorkspace, CommandPalette, Copy, Cut, DecreaseFontSize,
    FilePalette, FocusBrowser, FocusEditor, FocusNextPane, FocusTerminal, IncreaseFontSize,
    KeyboardHelp, NewTerminal, NextTab, NextWorkspace, OpenFile, Paste, PrevTab, PrevWorkspace,
    ResetFontSize, RunTask, Save, SelectAll, ToggleBrowser, ToggleEditor, ToggleSettings,
    ToggleSidebar, ToggleTerminal, XeroApp,
};

actions!(xero, [Quit]);

fn main() {
    init_logging();
    let paths = parse_cli_paths(std::env::args().skip(1));
    // Single-instance handoff: a live primary for this data dir takes the paths.
    if try_handoff(&paths) {
        return;
    }
    let listener = match bind_server() {
        Ok(l) => Some(l),
        Err(error) => {
            // Race: another primary bound first — hand off and exit.
            if try_handoff(&paths) {
                return;
            }
            log::warn!("ipc bind failed ({error}); continuing without single-instance");
            None
        }
    };

    let boot_paths = paths;
    application().run(move |cx: &mut App| {
        xero_terminal::init(cx);
        xero_ui::init(cx);
        wire_menus(cx);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                xero_terminal::observe_appearance(window, cx).detach();
                cx.new(move |cx| {
                    let mut app = XeroApp::new(cx);
                    if let Some(listener) = listener {
                        start_ipc(listener, cx);
                    }
                    for path in boot_paths {
                        app.open_cli_path(path, cx);
                    }
                    app
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

fn start_ipc(listener: std::os::unix::net::UnixListener, cx: &mut gpui::Context<XeroApp>) {
    let (tx, rx) = async_channel::unbounded::<IpcRequest>();
    serve_forever(listener, move |req| {
        let _ = tx.send_blocking(req);
    });
    cx.spawn(async move |app, cx| {
        while let Ok(req) = rx.recv().await {
            let result = app.update(cx, |app, cx| match req {
                IpcRequest::Open { paths } => {
                    for path in paths {
                        app.open_cli_path(path, cx);
                    }
                }
                IpcRequest::Activate => {}
            });
            if result.is_err() {
                break;
            }
            // Bring the shell forward after an external open request.
            cx.update(|cx| cx.activate(true));
        }
    })
    .detach();
}

/// Log under the active data dir (slot-aware via XENON_DATA_DIR / XERO_DATA_DIR),
/// with fallbacks for early boot. Bundled apps have no terminal for stderr.
/// `RUST_LOG` still overrides filters.
fn init_logging() {
    let dir = std::env::var_os("XENON_DATA_DIR")
        .or_else(|| std::env::var_os("XERO_DATA_DIR"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let home = PathBuf::from(home);
                if home.join(".xenon").exists() {
                    home.join(".xenon")
                } else {
                    home.join(".xero")
                }
            })
        })
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(
        &std::env::var("RUST_LOG").unwrap_or_else(|_| {
            "warn,xero=info,xero_ui=info,xero_ide=info,xero_terminal=info".into()
        }),
    );
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("xenon.log"))
    {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    let _ = builder.try_init();
}

/// App menus + Quit on Cmd-Q or closing the last window.
fn wire_menus(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.set_menus([
        Menu::new("Xenon").items([
            MenuItem::action("Preferences…", ToggleSettings),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Terminal", NewTerminal),
            MenuItem::separator(),
            MenuItem::action("Open File…", OpenFile),
            MenuItem::action("Open Workspace…", AddWorkspace),
            MenuItem::action("Go to File…", FilePalette),
            MenuItem::action("Command Palette…", CommandPalette),
            MenuItem::action("Run Task…", RunTask),
            MenuItem::separator(),
            MenuItem::action("Save", Save),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseEditor),
            MenuItem::action("Close Workspace", CloseWorkspace),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::separator(),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle File Browser", ToggleBrowser),
            MenuItem::action("Toggle Terminal", ToggleTerminal),
            MenuItem::action("Toggle Editor", ToggleEditor),
            MenuItem::separator(),
            MenuItem::action("Focus Terminal", FocusTerminal),
            MenuItem::action("Focus Editor", FocusEditor),
            MenuItem::action("Focus File Tree", FocusBrowser),
            MenuItem::action("Focus Next Pane", FocusNextPane),
            MenuItem::separator(),
            MenuItem::action("Next Workspace", NextWorkspace),
            MenuItem::action("Previous Workspace", PrevWorkspace),
            MenuItem::action("Next Tab", NextTab),
            MenuItem::action("Previous Tab", PrevTab),
            MenuItem::separator(),
            MenuItem::action("Keyboard Shortcuts", KeyboardHelp),
            MenuItem::separator(),
            MenuItem::action("Zoom In", IncreaseFontSize),
            MenuItem::action("Zoom Out", DecreaseFontSize),
            MenuItem::action("Reset Zoom", ResetFontSize),
        ]),
    ]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}
