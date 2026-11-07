//! Fixed choice lists for settings rows, and the font family lists a frontend
//! offers, built from whatever faces its text system reports.

use crate::{ProseFont, display_mono_family};

pub const PROSE_SIZES: [f32; 5] = [16.0, 17.0, 18.0, 20.0, 22.0];
pub const PROSE_LINE_HEIGHTS: [f32; 4] = [1.4, 1.55, 1.7, 1.9];
pub const PROSE_MEASURES: [f32; 4] = [56.0, 66.0, 76.0, 90.0];
pub const PROSE_FONTS: [ProseFont; 4] = [
    ProseFont::Auto,
    ProseFont::Serif,
    ProseFont::Sans,
    ProseFont::Mono,
];

/// Index of the value in `values` closest to `current`.
pub fn nearest_index(values: &[f32], current: f32) -> usize {
    values
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - current).abs().total_cmp(&(b.1 - current).abs()))
        .map_or(0, |(i, _)| i)
}

/// A size as shown: `14`, or `14.5`.
pub fn format_size(size: f32) -> String {
    if (size - size.round()).abs() < 0.01 {
        format!("{}", size.round() as i32)
    } else {
        format!("{size:.1}")
    }
}

/// One font family choice: the stored name and the label shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontChoice {
    pub value: String,
    pub label: String,
}

fn options(
    installed: impl IntoIterator<Item = String>,
    first: Option<&str>,
    current: &str,
    label: fn(&str) -> &str,
) -> Vec<FontChoice> {
    let mut names: Vec<String> = installed.into_iter().collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    for extra in [Some(current), first].into_iter().flatten() {
        if !extra.is_empty() && !names.iter().any(|n| n == extra) {
            names.insert(0, extra.to_string());
        }
    }
    if let Some(first) = first
        && let Some(at) = names.iter().position(|n| n == first)
    {
        let name = names.remove(at);
        names.insert(0, name);
    }
    names
        .into_iter()
        .map(|value| FontChoice {
            label: label(&value).to_string(),
            value,
        })
        .collect()
}

/// Editor and terminal faces: the monospaced families the caller found.
pub fn mono_font_choices(
    installed: impl IntoIterator<Item = String>,
    current: &str,
) -> Vec<FontChoice> {
    options(installed, None, current, display_mono_family)
}
