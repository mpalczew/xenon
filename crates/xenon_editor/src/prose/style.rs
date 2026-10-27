//! Prose colors. Paper is a document surface (not chrome): one warm palette
//! per appearance, kept here so it is the only place those hexes live. Without
//! Paper the prose view uses the active theme's own tokens.

use gpui::{Hsla, Rgba, rgb};
use theme::ThemeColors;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Hsla,
    pub ink: Hsla,
    /// Markdown markers and other syntax: at least 3:1 on `bg`.
    pub dim: Hsla,
    /// Done tasks and quotes.
    pub muted: Hsla,
    pub accent: Hsla,
    /// Code, table and chip tint.
    pub tint: Hsla,
    pub rule: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
}

/// Minimum contrast of marker ink against the page (WCAG non-text/large).
const MARKER_CONTRAST: f32 = 3.0;

pub fn palette(colors: &ThemeColors, light: bool, paper: bool, caret: Hsla) -> Palette {
    if paper {
        let (bg, ink, marker, tint, rule) = if light {
            (0xf8f4ec, 0x2b2722, 0xa39a8b, 0xf0eadf, 0xe0d8c8)
        } else {
            (0x16140f, 0xddd5c6, 0x6f685c, 0x1f1c15, 0x2e2a20)
        };
        let bg: Hsla = rgb(bg).into();
        let ink: Hsla = rgb(ink).into();
        let dim = at_least(rgb(marker).into(), ink, bg);
        Palette {
            bg,
            ink,
            dim,
            muted: ink.blend(bg.opacity(0.45)),
            accent: colors.text_accent,
            tint: rgb(tint).into(),
            rule: rgb(rule).into(),
            selection: colors.element_selected,
            caret,
        }
    } else {
        let bg = colors.editor_background;
        let ink = colors.editor_foreground;
        let dim = at_least(ink.blend(bg.opacity(0.5)), ink, bg);
        Palette {
            bg,
            ink,
            dim,
            muted: colors.text_muted,
            accent: colors.text_accent,
            tint: colors.surface_background,
            rule: colors.border,
            selection: colors.element_selected,
            caret,
        }
    }
}

/// Move `dim` toward `ink` until it reaches the marker contrast on `bg`.
fn at_least(dim: Hsla, ink: Hsla, bg: Hsla) -> Hsla {
    let mut color = dim;
    for step in 0..=10 {
        if contrast(color, bg) >= MARKER_CONTRAST {
            return color;
        }
        color = dim.blend(ink.opacity(step as f32 / 10.0));
    }
    ink
}

fn luminance(color: Hsla) -> f32 {
    let Rgba { r, g, b, .. } = color.into();
    let lin = |c: f32| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
