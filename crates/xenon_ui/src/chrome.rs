//! Shared chrome selection language for the anti-IDE shell.
//! Multi-channel always: fill + type (and optional edge). Never bg-only.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, Hsla, IntoElement, Styled, div, pulsating_between, px,
    transparent_black,
};
use theme::ThemeColors;

/// How a selectable chrome row or chip should paint.
#[derive(Clone, Copy)]
pub(crate) struct SelectionPaint {
    pub background: Hsla,
    pub foreground: Hsla,
    /// Edge accent (left bar, underline). Transparent when inactive.
    pub accent: Hsla,
}

/// A restrained accent wash for focused surfaces. The theme supplies both
/// the base and accent, so light, dark, and high-contrast palettes stay intact.
pub(crate) fn accent_surface(base: Hsla, accent: Hsla) -> Hsla {
    base.blend(accent.opacity(0.08))
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
pub(crate) fn tab_selection(colors: &ThemeColors, active: bool, focused: bool) -> SelectionPaint {
    let (background, foreground, accent) = if active && focused {
        let mut bg = colors.tab_active_background;
        // Light themes often use the canvas as the active tab surface. That
        // makes the active tab look like a hole in the shelf, so prefer the
        // theme's selected surface when the tokens collapse.
        if same_color(bg, colors.tab_inactive_background)
            || same_color(bg, colors.editor_background)
            || same_color(bg, colors.tab_bar_background)
        {
            bg = colors.element_selected;
        }
        bg = accent_surface(bg, colors.text_accent);
        (bg, colors.text, colors.text_accent)
    } else {
        (transparent_black(), colors.text_muted, transparent_black())
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

/// Sidebar fill. Canvas, not panel gray. Toolbar keeps `panel_background`.
pub(crate) fn sidebar_background(colors: &ThemeColors) -> Hsla {
    colors.editor_background
}

/// Attention / "needs you" indicator. Theme status.warning, not hard-coded amber.
pub(crate) fn attention_color(cx: &App) -> Hsla {
    use theme::ActiveTheme;
    cx.theme().status().warning
}

/// Working / "in progress" indicator. Theme status.info, not hard-coded blue.
pub(crate) fn working_color(cx: &App) -> Hsla {
    use theme::ActiveTheme;
    cx.theme().status().info
}

/// 6px status pip. Pulse is working; static is attention / dirty-style marks.
pub(crate) fn status_pip(color: Hsla, pulse: bool) -> impl IntoElement {
    let pip = div()
        .w(px(6.))
        .h(px(6.))
        .rounded_full()
        .bg(color)
        .flex_none();
    if pulse {
        pip.with_animation(
            "working-pulse",
            Animation::new(Duration::from_millis(1400))
                .repeat()
                .with_easing(pulsating_between(0.35, 1.0)),
            |this, delta| this.opacity(delta),
        )
        .into_any_element()
    } else {
        pip.into_any_element()
    }
}

fn same_color(a: Hsla, b: Hsla) -> bool {
    (a.h - b.h).abs() < 0.001
        && (a.s - b.s).abs() < 0.001
        && (a.l - b.l).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}
