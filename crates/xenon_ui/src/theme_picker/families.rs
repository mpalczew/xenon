//! The curated theme lineup: one row per family, dark and light halves.

use gpui::{App, SharedString};
use theme::{Appearance, ThemeRegistry};

pub(super) struct Family {
    pub name: &'static str,
    pub group: &'static str,
    pub dark: Option<&'static str>,
    pub light: Option<&'static str>,
}

impl Family {
    pub fn theme(&self, half: Appearance) -> Option<&'static str> {
        match half {
            Appearance::Dark => self.dark,
            Appearance::Light => self.light,
        }
    }

    /// The requested half, or the one this family has.
    pub fn nearest(&self, half: Appearance) -> (Appearance, &'static str) {
        match (self.theme(half), self.dark, self.light) {
            (Some(name), _, _) => (half, name),
            (None, Some(dark), _) => (Appearance::Dark, dark),
            (None, None, Some(light)) => (Appearance::Light, light),
            (None, None, None) => (half, ""),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.dark == Some(name) || self.light == Some(name)
    }
}

const fn family(
    name: &'static str,
    group: &'static str,
    dark: Option<&'static str>,
    light: Option<&'static str>,
) -> Family {
    Family {
        name,
        group,
        dark,
        light,
    }
}

/// Hot Dog Stand is bundled but left out: it is a hard-coded-color detector.
const LINEUP: [Family; 10] = [
    family("One", "Everyday", Some("One Dark"), Some("One Light")),
    family("Nord", "Everyday", Some("Nord"), Some("Nord Light")),
    family(
        "Solarized",
        "Everyday",
        Some("Solarized Dark"),
        Some("Solarized Light"),
    ),
    family(
        "High Contrast",
        "Everyday",
        Some("High Contrast Dark"),
        Some("High Contrast Light"),
    ),
    family("Neon", "Signature", Some("Neon Noir"), Some("Neon Mist")),
    family("Tokyo", "Signature", Some("Tokyo Night"), Some("Tokyo Day")),
    family(
        "Mithril",
        "Signature",
        Some("Mithril"),
        Some("Mithril Light"),
    ),
    family(
        "Imperial",
        "Signature",
        Some("Imperial"),
        Some("Imperial Light"),
    ),
    family(
        "Gruvbox",
        "Warm",
        Some("Gruvbox Dark"),
        Some("Gruvbox Light"),
    ),
    family("True Black", "OLED", Some("True Black"), None),
];

/// Families whose themes are registered (a missing file drops that half).
pub(super) fn lineup(cx: &App) -> Vec<Family> {
    let registry = ThemeRegistry::global(cx);
    let present = |name: Option<&'static str>| name.filter(|n| registry.get(n).is_ok());
    LINEUP
        .iter()
        .map(|f| family(f.name, f.group, present(f.dark), present(f.light)))
        .filter(|f| f.dark.is_some() || f.light.is_some())
        .collect()
}

pub(super) fn group_title(group: &str) -> SharedString {
    SharedString::from(group.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_only_family_falls_back_to_dark() {
        let black = &LINEUP[9];
        assert_eq!(
            black.nearest(Appearance::Light),
            (Appearance::Dark, "True Black")
        );
        assert!(black.contains("True Black"));
    }

    #[test]
    fn lineup_has_ten_families() {
        assert_eq!(LINEUP.len(), 10);
        assert!(LINEUP.iter().all(|f| f.dark.is_some()));
    }
}
