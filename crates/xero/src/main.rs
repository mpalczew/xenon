use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use xero_terminal::TerminalView;

fn main() {
    env_logger::init();
    application().run(|cx: &mut App| {
        xero_terminal::init(cx);

        let bounds = Bounds::centered(None, size(px(1024.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let cwd = std::env::current_dir().ok();
                cx.new(|cx| TerminalView::new(cwd, cx))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
