//! The curated theme lineup: one row per family, dark and light halves.

use gpui::{App, SharedString};
use theme::{Appearance, ThemeRegistry};

pub(super) struct Family {
    pub name: &'static str,
    pub group: &'static str,
    pub dark: Option<&'static str>,
    pub light: Option<&'static str>,
}

fn is_dark(half: Appearance) -> bool {
    half == Appearance::Dark
}

fn half_of(dark: bool) -> Appearance {
    if dark {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

impl Family {
    pub fn theme(&self, half: Appearance) -> Option<&'static str> {
        if is_dark(half) { self.dark } else { self.light }
    }

    /// The requested half, or the one this family has.
    pub fn nearest(&self, half: Appearance) -> (Appearance, &'static str) {
        let (dark, name) = xenon_settings::ThemeFamily {
            name: self.name,
            group: self.group,
            dark: self.dark,
            light: self.light,
        }
        .nearest(is_dark(half));
        (half_of(dark), name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.dark == Some(name) || self.light == Some(name)
    }
}

/// Families whose themes are registered (a missing file drops that half).
pub(super) fn lineup(cx: &App) -> Vec<Family> {
    let registry = ThemeRegistry::global(cx);
    let present = |name: Option<&'static str>| name.filter(|n| registry.get(n).is_ok());
    xenon_settings::THEME_LINEUP
        .iter()
        .map(|f| Family {
            name: f.name,
            group: f.group,
            dark: present(f.dark),
            light: present(f.light),
        })
        .filter(|f| f.dark.is_some() || f.light.is_some())
        .collect()
}

pub(super) fn group_title(group: &str) -> SharedString {
    SharedString::from(group.to_uppercase())
}
