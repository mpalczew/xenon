//! Phone remote scenes: Connect Phone sheet and Settings › Phone Remote.

use gpui::{Context, Window};

use super::{XenonApp, populate};

pub(super) fn connect_phone(
    app: &mut XenonApp,
    reachable: bool,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    app.visual_remote_listening(reachable, cx);
    app.open_connect_phone(cx);
}

pub(super) fn settings_remote(app: &mut XenonApp, cx: &mut Context<XenonApp>) {
    app.visual_remote(true, cx);
    app.open_settings_window_now(cx);
}
