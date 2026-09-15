//! Shared visual vocabulary for Xenon's native shell.
//!
//! Feature composition belongs in `xenon_ui`; this crate owns reusable
//! selection, status, elevation, and motion primitives.

use std::time::Duration;

#[cfg(feature = "visual-tests")]
use gpui::Global;
use gpui::{
    Animation, AnimationExt, App, Hsla, IntoElement, Styled, div, pulsating_between, px,
    transparent_black,
};
use theme::ThemeColors;

/// How a selectable chrome row or chip should paint.
#[derive(Clone, Copy)]
pub struct SelectionPaint {
    pub background: Hsla,
    pub foreground: Hsla,
    /// Edge accent (left bar, underline). Transparent when inactive.
    pub accent: Hsla,
}

/// A restrained accent wash for focused surfaces.
pub fn accent_surface(base: Hsla, accent: Hsla) -> Hsla {
    base.blend(accent.opacity(0.08))
}

/// List / nav row selection: fill + type + edge.
pub fn list_selection(colors: &ThemeColors, active: bool) -> SelectionPaint {
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

/// Tab selection with a visible edge even when light-theme tab surfaces blend.
pub fn tab_selection(colors: &ThemeColors, active: bool, focused: bool) -> SelectionPaint {
    let (background, foreground, accent) = if active && focused {
        let mut bg = colors.tab_active_background;
        if same_color(bg, colors.tab_inactive_background)
            || same_color(bg, colors.editor_background)
            || same_color(bg, colors.tab_bar_background)
        {
            bg = colors.element_selected;
        }
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

pub fn attention_color(cx: &App) -> Hsla {
    use theme::ActiveTheme;
    cx.theme().status().warning
}

pub fn working_color(cx: &App) -> Hsla {
    use theme::ActiveTheme;
    cx.theme().status().info
}

#[cfg(feature = "visual-tests")]
struct MotionFrozen;

#[cfg(feature = "visual-tests")]
impl Global for MotionFrozen {}

/// Stop wall-clock chrome animation during deterministic visual captures.
#[cfg(feature = "visual-tests")]
pub fn freeze_motion(cx: &mut App) {
    cx.set_global(MotionFrozen);
}

fn motion_frozen(cx: &App) -> bool {
    #[cfg(feature = "visual-tests")]
    {
        cx.has_global::<MotionFrozen>()
    }
    #[cfg(not(feature = "visual-tests"))]
    {
        let _ = cx;
        false
    }
}

/// Six-pixel status pip. Working pulses; attention stays static.
pub fn status_pip(color: Hsla, pulse: bool, cx: &App) -> impl IntoElement {
    let pip = div()
        .w(px(6.))
        .h(px(6.))
        .rounded_full()
        .bg(color)
        .flex_none();
    if pulse && !motion_frozen(cx) {
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
