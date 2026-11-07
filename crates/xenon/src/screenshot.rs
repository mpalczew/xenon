//! Offscreen window capture: the window's render node, drawn by GTK's own
//! renderer to a texture and saved as PNG. Works the same on Wayland and X11,
//! with no compositor screenshot tool. It does not depend on the compositor
//! sending frame callbacks, so it works for an unfocused or hidden window.

use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

/// Render the window to `path`, then call `done` with the outcome.
pub(crate) fn render_png(
    win: &adw::ApplicationWindow,
    path: &str,
    done: impl FnOnce(Result<(), String>) + 'static,
) {
    // Let pending layout and style work finish, then snapshot the content.
    let (win, path) = (win.clone(), path.to_string());
    glib::timeout_add_local_once(Duration::from_millis(250), move || {
        done(capture(&win, &path));
    });
}

fn capture(win: &adw::ApplicationWindow, path: &str) -> Result<(), String> {
    // The whole window, so an open dialog is in the picture too.
    let (w, h) = (win.width(), win.height());
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(win)).snapshot(&snapshot, f64::from(w), f64::from(h));
    let node = snapshot
        .to_node()
        .ok_or_else(|| format!("empty render node ({w}x{h})"))?;
    let renderer = win
        .native()
        .and_then(|n| n.renderer())
        .ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    texture.save_to_png(path).map_err(|e| e.to_string())
}
