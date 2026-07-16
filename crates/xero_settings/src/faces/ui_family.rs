//! UI chrome font family resolve + display labels.

use gpui::App;
use xero_store::DEFAULT_UI_FONT_FAMILY;

/// Resolve a UI face name; fall back to system UI font when unusable.
pub fn ensure_ui_family(family: &str, cx: &App) -> String {
    let candidate = family.trim();
    if candidate.is_empty() {
        return DEFAULT_UI_FONT_FAMILY.into();
    }
    if ui_family_resolves(candidate, cx) {
        return candidate.to_string();
    }
    if candidate != DEFAULT_UI_FONT_FAMILY && ui_family_resolves(DEFAULT_UI_FONT_FAMILY, cx) {
        log::warn!("ui font '{family}' unavailable; using {DEFAULT_UI_FONT_FAMILY}");
        return DEFAULT_UI_FONT_FAMILY.into();
    }
    candidate.to_string()
}

/// Human label for a stored UI family (settings dropdown).
pub fn display_ui_family(family: &str) -> &str {
    match family {
        ".SystemUIFont" | ".AppleSystemUIFont" => "System",
        other => other,
    }
}

fn ui_family_resolves(family: &str, cx: &App) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let font = gpui::font(family);
        let font_id = cx.text_system().resolve_font(&font);
        cx.text_system()
            .advance(font_id, gpui::px(14.), 'M')
            .is_ok()
    }))
    .unwrap_or(false)
}
