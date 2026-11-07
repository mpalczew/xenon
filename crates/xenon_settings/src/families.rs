//! Font family names: aliases, display labels, and healing against what the
//! frontend's text system can actually draw.

use xenon_store::{DEFAULT_FONT_FAMILY, DEFAULT_UI_FONT_FAMILY};

/// What a frontend can tell us about installed faces.
pub trait FontProbe {
    /// True when `family` resolves and has equal advances for i/W/m.
    fn is_monospace(&self, family: &str) -> bool;
    /// True when `family` resolves to a drawable face.
    fn resolves(&self, family: &str) -> bool;
}

/// Map marketing / legacy names to Core Text family names.
pub(crate) fn canonicalize_mono_family(family: &str) -> String {
    match family.trim() {
        "SF Mono" | "SFMono" | "SFMono-Regular" => ".SF NS Mono".into(),
        "Zed Mono" | ".ZedMono" | "Zed Plex Mono" => "Lilex".into(),
        other => other.to_string(),
    }
}

/// Human label for a stored family (settings dropdown trigger).
pub(crate) fn display_mono_family(family: &str) -> &str {
    match family {
        ".SF NS Mono" => "SF Mono",
        other => other,
    }
}

/// Resolve aliases and require a monospaced face; otherwise fall back to Lilex.
pub(crate) fn ensure_mono_family(family: &str, probe: &dyn FontProbe) -> String {
    let candidate = canonicalize_mono_family(family);
    if probe.is_monospace(&candidate) {
        return candidate;
    }
    if candidate != DEFAULT_FONT_FAMILY && probe.is_monospace(DEFAULT_FONT_FAMILY) {
        log::warn!(
            "font family '{family}' is not a usable monospace face; using {DEFAULT_FONT_FAMILY}"
        );
        return DEFAULT_FONT_FAMILY.into();
    }
    candidate
}

/// Resolve a UI face name; fall back to system UI font when unusable.
pub(crate) fn ensure_ui_family(family: &str, probe: &dyn FontProbe) -> String {
    let candidate = family.trim();
    if candidate.is_empty() {
        return DEFAULT_UI_FONT_FAMILY.into();
    }
    if probe.resolves(candidate) {
        return candidate.to_string();
    }
    if candidate != DEFAULT_UI_FONT_FAMILY && probe.resolves(DEFAULT_UI_FONT_FAMILY) {
        log::warn!("ui font '{family}' unavailable; using {DEFAULT_UI_FONT_FAMILY}");
        return DEFAULT_UI_FONT_FAMILY.into();
    }
    candidate.to_string()
}
