use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions, actions, px,
    size,
};
use gpui_platform::application;
use xero_ui::XeroApp;

actions!(xero, [Quit]);

fn main() {
    env_logger::init();
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
            |_, cx| cx.new(XeroApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
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
