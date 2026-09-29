//! Phone remote scenes: the Connect Phone sheet.

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
