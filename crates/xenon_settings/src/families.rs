use xenon_store::{DEFAULT_FONT_FAMILY, DEFAULT_UI_FONT_FAMILY};

pub trait FontProbe {
    /// True when `family` resolves and has equal advances for i/W/m.
    fn is_monospace(&self, family: &str) -> bool;
    /// True when `family` resolves to a drawable face.
    fn resolves(&self, family: &str) -> bool;
}

pub(crate) fn ensure_mono_family(family: &str, probe: &dyn FontProbe) -> String {
    let candidate = family.trim().to_string();
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
