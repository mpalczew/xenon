//! Celebratory empty state: a large check that draws itself over a soft glow.

use std::{sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ElementId, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, Stateful, Styled, div, point, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::{TypeRole, Typography};

const MARK: f32 = 64.;
const GLYPH: f32 = 36.;

pub struct AllClear {
    pub title: SharedString,
    pub detail: SharedString,
}

/// Centered column that fills its parent. Callers append key hints.
pub fn all_clear(id: impl Into<ElementId>, copy: AllClear, cx: &App) -> gpui::Div {
    let id = id.into();
    let accent = cx.theme().colors().text_accent;
    let animate = !super::motion_frozen(cx) && !super::reduce_motion();
    div()
        .relative()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(glow(&id, accent, animate))
        .child(mark(&id, cx, animate))
        .child(
            div()
                .mt_2()
                .type_role(TypeRole::ScreenTitle, cx)
                .child(copy.title),
        )
        .child(div().type_role(TypeRole::Supporting, cx).child(copy.detail))
}

/// Pill with a keycap and a short label. Callers add `.on_click` when it acts.
pub fn key_chip(
    id: impl Into<ElementId>,
    keys: &'static str,
    label: impl Into<SharedString>,
    cx: &App,
) -> Stateful<gpui::Div> {
    let colors = cx.theme().colors();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1p5()
        .pl(px(6.))
        .pr(px(10.))
        .py(px(4.))
        .rounded_full()
        .border_1()
        .border_color(colors.border)
        .bg(colors.editor_background)
        .type_role(TypeRole::ControlLabel, cx)
        .child(
            div()
                .px(px(5.))
                .rounded(px(4.))
                .border_1()
                .border_b_2()
                .border_color(colors.border)
                .bg(colors.element_background)
                .text_color(colors.text)
                .child(xenon_keymap::display_keys(keys)),
        )
        .child(label.into())
}

fn glow(id: &ElementId, accent: Hsla, animate: bool) -> AnyElement {
    let halo = div().size(px(160.)).rounded_full().shadow(vec![BoxShadow {
        color: accent.opacity(0.2),
        offset: point(px(0.), px(0.)),
        blur_radius: px(140.),
        spread_radius: px(60.),
        inset: false,
    }]);
    let halo = if animate {
        halo.with_animation(
            child_id(id, "glow"),
            Animation::new(Duration::from_millis(5000))
                .repeat()
                .with_easing(|t| (t * std::f32::consts::TAU).cos().mul_add(0.15, 0.85)),
            |halo, t| halo.opacity(t),
        )
        .into_any_element()
    } else {
        halo.into_any_element()
    };
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(halo)
        .into_any_element()
}

fn mark(id: &ElementId, cx: &App, animate: bool) -> impl IntoElement {
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    let face = div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(16.))
        .border_2()
        .border_color(accent)
        .bg(colors.editor_background)
        .child(stroke(id, accent, animate));
    div()
        .relative()
        .size(px(MARK))
        .flex_none()
        .children(animate.then(|| ring(id, accent)))
        .child(face)
}

/// The check wipes in left to right, like a pen stroke.
fn stroke(id: &ElementId, accent: Hsla, animate: bool) -> impl IntoElement {
    let glyph = div()
        .w(px(GLYPH))
        .flex_none()
        .font_family("lucide")
        .text_size(px(GLYPH))
        .line_height(px(GLYPH))
        .text_color(accent)
        .child(char::from(Icon::Check).to_string());
    let clip = div()
        .absolute()
        .top_0()
        .left_0()
        .h(px(GLYPH))
        .w(px(GLYPH))
        .overflow_hidden()
        .child(glyph);
    let clip = if animate {
        clip.with_animation(
            child_id(id, "stroke"),
            Animation::new(Duration::from_millis(1000)),
            |clip, t| clip.w(px(GLYPH * draw_progress(t))),
        )
        .into_any_element()
    } else {
        clip.into_any_element()
    };
    div().relative().size(px(GLYPH)).child(clip)
}

/// A ring expands from the frame once the stroke lands.
fn ring(id: &ElementId, accent: Hsla) -> impl IntoElement {
    div().absolute().inset_0().rounded(px(16.)).with_animation(
        child_id(id, "ring"),
        Animation::new(Duration::from_millis(1600)),
        move |ring, t| {
            let spread = ring_progress(t);
            ring.shadow(vec![BoxShadow {
                color: accent.opacity(0.5 * (1. - spread)),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(18. * spread),
                inset: false,
            }])
        },
    )
}

/// 300ms rest, then a 700ms ease-out draw.
fn draw_progress(t: f32) -> f32 {
    let local = ((t * 1000. - 300.) / 700.).clamp(0., 1.);
    1. - (1. - local).powi(3)
}

/// Starts as the stroke lands (700ms) and runs 900ms.
fn ring_progress(t: f32) -> f32 {
    ((t * 1600. - 700.) / 900.).clamp(0., 1.)
}

fn child_id(id: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(Arc::new(id.clone()), name.into())
}

#[cfg(test)]
mod tests {
    use super::{draw_progress, ring_progress};

    #[test]
    fn stroke_waits_then_finishes_drawn() {
        assert_eq!(draw_progress(0.2), 0.);
        assert!(draw_progress(0.6) > 0.);
        assert!((draw_progress(1.) - 1.).abs() < 0.001);
    }

    #[test]
    fn ring_starts_after_the_stroke_lands() {
        assert_eq!(ring_progress(0.4), 0.);
        assert!((ring_progress(1.) - 1.).abs() < 0.001);
    }
}
