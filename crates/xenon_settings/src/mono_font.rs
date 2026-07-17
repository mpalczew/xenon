//! Resolve and validate monospaced face names for editor/terminal.

use gpui::{App, px};
use xenon_store::DEFAULT_FONT_FAMILY;

/// Map marketing / legacy names to Core Text family names.
pub fn canonicalize(family: &str) -> String {
    match family.trim() {
        "SF Mono" | "SFMono" | "SFMono-Regular" => ".SF NS Mono".into(),
        other => other.to_string(),
    }
}

/// Human label for a stored family (settings dropdown trigger).
pub fn display_name(family: &str) -> &str {
    match family {
        ".SF NS Mono" => "SF Mono",
        other => other,
    }
}

/// Resolve aliases and require a monospaced face; otherwise fall back to Menlo.
pub fn ensure(family: &str, cx: &App) -> String {
    let candidate = canonicalize(family);
    if is_safe(&candidate, cx) {
        return candidate;
    }
    if candidate != DEFAULT_FONT_FAMILY && is_safe(DEFAULT_FONT_FAMILY, cx) {
        log::warn!(
            "font family '{family}' is not a usable monospace face; using {DEFAULT_FONT_FAMILY}"
        );
        return DEFAULT_FONT_FAMILY.into();
    }
    candidate
}

/// True when `family` resolves and has equal advances for i/W/m.
pub fn is_family(family: &str, cx: &App) -> bool {
    is_safe(family, cx)
}

fn is_safe(family: &str, cx: &App) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| is_mono(family, cx))).unwrap_or(false)
}

fn is_mono(family: &str, cx: &App) -> bool {
    // resolve_font fails open to Helvetica etc.; equal advances filter those out.
    let font = gpui::font(family);
    let font_id = cx.text_system().resolve_font(&font);
    let size = px(14.);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_sf_mono() {
        assert_eq!(canonicalize("SF Mono"), ".SF NS Mono");
        assert_eq!(canonicalize("SFMono"), ".SF NS Mono");
        assert_eq!(canonicalize("Menlo"), "Menlo");
    }

    #[test]
    fn display_sf_mono() {
        assert_eq!(display_name(".SF NS Mono"), "SF Mono");
        assert_eq!(display_name("Menlo"), "Menlo");
    }
}
