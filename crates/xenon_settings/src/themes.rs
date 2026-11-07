//! The curated theme lineup and the short swatch strips, as plain names. A
//! frontend keeps only the names whose theme files it actually loaded.

/// One theme family: a dark and a light half (either may be missing).
pub struct ThemeFamily {
    pub name: &'static str,
    pub group: &'static str,
    pub dark: Option<&'static str>,
    pub light: Option<&'static str>,
}

impl ThemeFamily {
    pub fn theme(&self, dark: bool) -> Option<&'static str> {
        if dark { self.dark } else { self.light }
    }

    /// The requested half, or the one this family has. Returns `(is_dark, name)`.
    pub fn nearest(&self, dark: bool) -> (bool, &'static str) {
        match (self.theme(dark), self.dark, self.light) {
            (Some(name), _, _) => (dark, name),
            (None, Some(name), _) => (true, name),
            (None, None, Some(name)) => (false, name),
            (None, None, None) => (dark, ""),
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
) -> ThemeFamily {
    ThemeFamily {
        name,
        group,
        dark,
        light,
    }
}

/// Hot Dog Stand is bundled but left out: it is a hard-coded-color detector.
pub const THEME_LINEUP: [ThemeFamily; 10] = [
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

const DARK_PICKS: [&str; 5] = ["One Dark", "Neon Noir", "Nord", "Tokyo Night", "True Black"];
const LIGHT_PICKS: [&str; 5] = [
    "One Light",
    "Solarized Light",
    "Nord Light",
    "Tokyo Day",
    "High Contrast Light",
];

/// The five names in the Appearance swatch strip; the current theme takes the
/// last slot's place (shown first) when it is not already a pick.
pub fn swatch_names(dark: bool, current: &str) -> Vec<&str> {
    let mut names: Vec<&str> = if dark { DARK_PICKS } else { LIGHT_PICKS }.to_vec();
    if !names.contains(&current) {
        names.pop();
        names.insert(0, current);
    }
    names
}
