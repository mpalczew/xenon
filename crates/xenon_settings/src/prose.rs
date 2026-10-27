//! Markdown prose-mode look (markers, face, size, line height, measure) as a
//! gpui global, persisted in `settings.json`.

use gpui::{App, Global};
use xenon_store::{AppSettings, ProseFont, ProseMarkers, ProseSettings};

#[derive(Clone)]
struct Prose(ProseSettings);
impl Global for Prose {}

pub(super) fn apply(settings: &AppSettings, cx: &mut App) {
    cx.set_global(Prose(sanitize(settings.prose.clone())));
}

pub(super) fn write(settings: &mut AppSettings, cx: &App) {
    settings.prose = prose(cx);
}

fn sanitize(mut value: ProseSettings) -> ProseSettings {
    value.size = value.size.clamp(12.0, 36.0);
    value.line_height = value.line_height.clamp(1.2, 2.2);
    value.measure = value.measure.clamp(40.0, 120.0);
    value
}

/// Current prose look.
pub fn prose(cx: &App) -> ProseSettings {
    cx.try_global::<Prose>()
        .map(|p| p.0.clone())
        .unwrap_or_default()
}

/// Change the prose look and persist it.
pub fn update_prose(cx: &mut App, change: impl FnOnce(&mut ProseSettings)) {
    let mut value = prose(cx);
    change(&mut value);
    cx.set_global(Prose(sanitize(value)));
    super::save(cx);
}

pub fn prose_markers(cx: &App) -> ProseMarkers {
    prose(cx).markers
}

pub fn prose_font(cx: &App) -> ProseFont {
    prose(cx).font
}
