//! Shared chrome selection language for the anti-IDE shell.
//! Multi-channel always: fill + type (and optional edge). Never bg-only.

use gpui::{Hsla, transparent_black};
use theme::ThemeColors;

/// How a selectable chrome row or chip should paint.
#[derive(Clone, Copy)]
pub(crate) struct SelectionPaint {
    pub background: Hsla,
    pub foreground: Hsla,
    /// Edge accent (left bar, underline). Transparent when inactive.
    pub accent: Hsla,
}

/// List / nav row (workspaces, finder, tree, toolbar toggles).
pub(crate) fn list_selection(colors: &ThemeColors, active: bool) -> SelectionPaint {
    if active {
        SelectionPaint {
            background: colors.element_selected,
            foreground: colors.text,
            accent: colors.border_selected,
        }
    } else {
        SelectionPaint {
            background: transparent_black(),
            foreground: colors.text_muted,
            accent: transparent_black(),
        }
    }
}

/// Tab chip: prefers theme tab tokens; falls back when they collapse.
pub(crate) fn tab_selection(colors: &ThemeColors, active: bool) -> SelectionPaint {
    let (background, foreground, accent) = if active {
        let mut bg = colors.tab_active_background;
        if same_color(bg, colors.tab_inactive_background) {
            bg = colors.element_selected;
        }
        (bg, colors.text, colors.text_accent)
    } else {
        (
            colors.tab_inactive_background,
            colors.text_muted,
            transparent_black(),
        )
    };
    SelectionPaint {
        background,
        foreground,
        accent,
    }
}

pub(crate) fn tab_bar_background(colors: &ThemeColors) -> Hsla {
    colors.tab_bar_background
}

/// Attention / "needs you" indicator. Theme status.warning, not hard-coded amber.
pub(crate) fn attention_color(cx: &gpui::App) -> Hsla {
    use theme::ActiveTheme;
    cx.theme().status().warning
}

fn same_color(a: Hsla, b: Hsla) -> bool {
    (a.h - b.h).abs() < 0.001
        && (a.s - b.s).abs() < 0.001
        && (a.l - b.l).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}
