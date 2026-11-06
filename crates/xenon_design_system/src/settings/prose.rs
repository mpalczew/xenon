//! Markdown prose look.

use gpui::App;
use xenon_settings::{ProseFont, ProseMarkers, ProseSettings};

use super::{change, get, save};

/// Current prose look.
pub fn prose(cx: &App) -> ProseSettings {
    get(cx).prose
}

/// Change the prose look and persist it.
pub fn update_prose(cx: &mut App, edit: impl FnOnce(&mut ProseSettings)) {
    change(cx, |s| s.update_prose(edit));
    save(cx);
}

pub fn prose_markers(cx: &App) -> ProseMarkers {
    prose(cx).markers
}

pub fn prose_font(cx: &App) -> ProseFont {
    prose(cx).font
}

/// Nudge the markdown prose body size (cmd-+ / cmd-- in prose).
pub fn nudge_prose_font_size(cx: &mut App, delta: f32) {
    update_prose(cx, |p| p.size += delta);
}

/// Restore the default markdown prose body size.
pub fn reset_prose_font_size(cx: &mut App) {
    let size = ProseSettings::default().size;
    update_prose(cx, |p| p.size = size);
}
