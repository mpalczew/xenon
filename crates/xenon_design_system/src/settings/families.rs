//! Font families: probing GPUI's text system, and the per-surface faces.

use gpui::App;
use xenon_settings::{FaceFont, FontProbe};

use super::get;

pub(super) struct GpuiFonts<'a>(pub(super) &'a App);

impl FontProbe for GpuiFonts<'_> {
    fn is_monospace(&self, family: &str) -> bool {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| is_mono(family, self.0)))
            .unwrap_or(false)
    }

    fn resolves(&self, family: &str) -> bool {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let font = gpui::font(family);
            let font_id = self.0.text_system().resolve_font(&font);
            self.0
                .text_system()
                .advance(font_id, gpui::px(14.), 'M')
                .is_ok()
        }))
        .unwrap_or(false)
    }
}

fn is_mono(family: &str, cx: &App) -> bool {
    // resolve_font fails open to Helvetica etc.; equal advances filter those out.
    let font = gpui::font(family);
    let font_id = cx.text_system().resolve_font(&font);
    let size = gpui::px(14.);
    let Ok(i) = cx.text_system().advance(font_id, size, 'i') else {
        return false;
    };
    let Ok(w) = cx.text_system().advance(font_id, size, 'W') else {
        return false;
    };
    let Ok(m) = cx.text_system().advance(font_id, size, 'm') else {
        return false;
    };
    let wi = f32::from(i.width);
    let ww = f32::from(w.width);
    let wm = f32::from(m.width);
    (wi - ww).abs() < 0.5 && (wi - wm).abs() < 0.5 && wi > 0.0
}

/// Resolve aliases and require a monospaced face; otherwise fall back to Lilex.
pub fn ensure_mono_family(family: &str, cx: &App) -> String {
    xenon_settings::ensure_mono_family(family, &GpuiFonts(cx))
}

/// True when `family` resolves and has equal advances for i/W/m.
pub fn is_monospace_family(family: &str, cx: &App) -> bool {
    GpuiFonts(cx).is_monospace(family)
}

/// Resolve a UI face name; fall back to system UI font when unusable.
pub fn ensure_ui_family(family: &str, cx: &App) -> String {
    xenon_settings::ensure_ui_family(family, &GpuiFonts(cx))
}

pub fn editor_font(cx: &App) -> FaceFont {
    get(cx).editor
}

pub fn terminal_font(cx: &App) -> FaceFont {
    get(cx).terminal
}

pub fn ui_font(cx: &App) -> FaceFont {
    get(cx).ui
}
