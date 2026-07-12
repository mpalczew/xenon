use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, OsAction, WindowBounds, WindowOptions,
    actions, px, size,
};
use gpui_platform::application;
use xero_ui::{
    AddWorkspace, CloseEditor, Copy, Cut, FilePalette, OpenFile, Paste, RunTask, Save,
    ToggleSettings, XeroApp,
};

actions!(xero, [Quit]);

fn main() {
    init_logging();
    application().run(|cx: &mut App| {
        xero_terminal::init(cx);
        xero_ui::init(cx);
        wire_menus(cx);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                xero_terminal::observe_appearance(window, cx).detach();
                cx.new(XeroApp::new)
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

/// Log to `~/.xero/xero.log` (bundled apps have no terminal for stderr), at
/// info for xero crates. `RUST_LOG` still overrides.
fn init_logging() {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let dir = home.join(".xero");
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
        .open(dir.join("xero.log"))
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
        Menu::new("xero").items([
            MenuItem::action("Preferences…", ToggleSettings),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open File…", OpenFile),
            MenuItem::action("Open Folder…", AddWorkspace),
            MenuItem::action("Go to File…", FilePalette),
            MenuItem::action("Run Task…", RunTask),
            MenuItem::separator(),
            MenuItem::action("Save", Save),
            MenuItem::separator(),
            MenuItem::action("Close Editor", CloseEditor),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
        ]),
    ]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}
