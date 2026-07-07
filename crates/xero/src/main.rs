use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions, px,
    size,
};
use gpui_platform::application;
use xero_ui::XeroApp;

actions!(xero, [Quit]);

fn main() {
    init_logging();
    application().run(|cx: &mut App| {
        xero_terminal::init(cx);
        xero_ui::bind_keys(cx);
        wire_quit(cx);

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
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let dir = home.join(".xero");
    let _ = std::fs::create_dir_all(&dir);
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(&std::env::var("RUST_LOG").unwrap_or_else(|_| "warn,xero=info,xero_ui=info,xero_ide=info,xero_terminal=info".into()));
    if let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("xero.log")) {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    let _ = builder.try_init();
}

/// Quit on Cmd-Q, the app menu, or closing the last window.
fn wire_quit(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.set_menus([Menu::new("xero").items([MenuItem::action("Quit", Quit)])]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}
