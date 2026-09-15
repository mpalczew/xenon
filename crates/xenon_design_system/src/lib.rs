//! Shared visual vocabulary for Xenon's native shell.
//!
//! Feature composition belongs in `xenon_ui`; this crate owns reusable
//! selection, status, elevation, and motion primitives.

use std::time::Duration;

#[cfg(feature = "visual-tests")]
use gpui::Global;
use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, Hsla, IntoElement, Styled, px,
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

pub fn status_color(cx: &App, attention: bool) -> Hsla {
    use theme::ActiveTheme;
    if attention {
        cx.theme().status().warning
    } else {
        cx.theme().status().info
    }
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

/// Fade content into an expanding shell section without delaying interaction.
pub fn section_reveal(
    element: impl IntoElement + Styled + 'static,
    id: impl Into<ElementId>,
    cx: &App,
    opening: bool,
) -> AnyElement {
    if motion_frozen(cx) {
        return element.into_any_element();
    }
    element
        .with_animation(
            id,
            Animation::new(Duration::from_millis(180))
                .with_easing(|delta| delta * delta * (3. - 2. * delta)),
            move |this, delta| {
                let progress = if opening { delta } else { 1. - delta };
                this.opacity(progress).pt(px(6. * (1. - progress)))
            },
        )
        .into_any_element()
}

fn same_color(a: Hsla, b: Hsla) -> bool {
    (a.h - b.h).abs() < 0.001
        && (a.s - b.s).abs() < 0.001
        && (a.l - b.l).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}
