use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions, px,
    size,
};
use gpui_platform::application;
use xero_editor::EditorView;
use xero_terminal::TerminalView;

actions!(xero, [Quit]);

fn main() {
    env_logger::init();
    application().run(|cx: &mut App| {
        xero_terminal::init(cx);
        wire_quit(cx);

        let bounds = Bounds::centered(None, size(px(1024.), px(720.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };

        // A file path argument opens the editor; otherwise a terminal.
        match std::env::args().nth(1).map(PathBuf::from) {
            Some(path) => open_editor(path, options, cx),
            None => open_terminal(options, cx),
        }
        cx.activate(true);
    });
}

fn open_terminal(options: WindowOptions, cx: &mut App) {
    cx.open_window(options, |_, cx| {
        let cwd = std::env::current_dir().ok();
        cx.new(|cx| TerminalView::new(cwd, cx))
    })
    .unwrap();
}

fn open_editor(path: PathBuf, options: WindowOptions, cx: &mut App) {
    let result = cx.open_window(options, |_, cx| {
        cx.new(|cx| EditorView::open(path.clone(), cx).expect("failed to open file"))
    });
    if let Err(error) = result {
        eprintln!("could not open window: {error}");
    }
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
