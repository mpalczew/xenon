//! Soft wrap defaults: prose on, code off. The buffer is not rewritten.

use std::path::Path;

use gpui::{App, Global};
use xenon_store::AppSettings;

#[derive(Copy, Clone)]
struct WrapMode {
    prose: bool,
    code: bool,
}
impl Global for WrapMode {}

pub(super) fn apply(settings: &AppSettings, cx: &mut App) {
    cx.set_global(WrapMode {
        prose: settings.wrap_prose,
        code: settings.wrap_code,
    });
}

pub(super) fn write(settings: &mut AppSettings, cx: &App) {
    let mode = current(cx);
    settings.wrap_prose = mode.prose;
    settings.wrap_code = mode.code;
}

fn current(cx: &App) -> WrapMode {
    cx.try_global::<WrapMode>().copied().unwrap_or(WrapMode {
        prose: true,
        code: false,
    })
}

fn prose_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx" | "txt" | "rst" | "adoc")
    )
}

/// Whether this file should soft-wrap. Prose defaults on; code defaults off.
pub fn editor_wraps(path: &Path, cx: &App) -> bool {
    let mode = current(cx);
    if prose_path(path) {
        mode.prose
    } else {
        mode.code
    }
}

/// Flip wrap for the open file. No path flips the prose default.
pub fn toggle_editor_wrap(path: Option<&Path>, cx: &mut App) {
    let mut mode = current(cx);
    if path.is_none_or(prose_path) {
        mode.prose = !mode.prose;
    } else {
        mode.code = !mode.code;
    }
    cx.set_global(mode);
}
