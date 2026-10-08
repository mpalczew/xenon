//! The island's pixels: surface, words, countdown chip, and arrival motion.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Context, ElementId, Hsla, InteractiveElement,
    IntoElement, ParentElement, PathBuilder, Pixels, StatefulInteractiveElement, Styled, Window,
    canvas, div, point, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use super::view::Shown;
use super::{Toast, ToastAction, ToastKind, ToastView};
use crate::{Shortcut, TypeRole, Typography};

/// Ring diameter and key size by shortcut length, so ⌘⌥K fits as well as ⌘Z.
/// Ring size and key text size; `None` when the text is too long to sit
/// inside the ring (spelled-out keys like `Ctrl+Z`) and goes beside it.
fn ring_metrics(keys: &str) -> (f32, Option<f32>) {
    match keys.chars().count() {
        0..=2 => (24., Some(9.)),
        3 => (30., Some(8.5)),
        _ => (22., None),
    }
}

pub(super) fn island(shown: &Shown, cx: &mut Context<ToastView>) -> gpui::Stateful<gpui::Div> {
    let colors = cx.theme().colors();
    let toast = &shown.toast;
    let error = cx.theme().status().error;
    let (background, border) = match toast.kind {
        ToastKind::Error => (
            colors
                .elevated_surface_background
                .blend(error.opacity(0.12)),
            error.opacity(0.65),
        ),
        _ => (colors.elevated_surface_background, colors.border),
    };
    div()
        .id(("toast", shown.generation))
        .flex()
        .items_center()
        .gap(px(9.))
        .max_w(px(460.))
        .pl(px(12.))
        .pr(px(6.))
        .py(px(6.))
        .bg(background)
        .border_l_1()
        .border_r_1()
        .border_b_1()
        .border_color(border)
        .rounded_b(px(16.))
        .shadow_lg()
        .on_hover(cx.listener(|this, hovered: &bool, _, cx| this.set_hovered(*hovered, cx)))
        .child(div().flex_none().text_size(px(17.)).child(toast.glyph))
        .child(words(toast, cx))
        .children(
            toast
                .action
                .as_ref()
                .map(|action| chip(action, shown.progress(), cx)),
        )
        .children(toast.sticky().then(|| close(cx)))
}

fn words(toast: &Toast, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .pr(px(4.))
        .child(
            div()
                .type_role(TypeRole::ListPrimary, cx)
                .child(toast.title.clone()),
        )
        .children(
            toast
                .detail
                .clone()
                .map(|detail| div().type_role(TypeRole::ControlLabel, cx).child(detail)),
        )
}

fn chip(action: &ToastAction, progress: f32, cx: &mut Context<ToastView>) -> impl IntoElement {
    let colors = cx.theme().colors();
    let keys = action.shortcut.keys();
    let beside = ring_metrics(&keys).1.is_none().then(|| {
        div()
            .text_size(px(10.))
            .text_color(colors.text_muted)
            .child(keys)
    });
    div()
        .id("toast-action")
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(3.))
        .pr(px(10.))
        .py(px(3.))
        .rounded_full()
        .bg(colors.element_selected)
        .hover(|style| style.bg(colors.element_hover))
        .cursor_pointer()
        .on_click(cx.listener(|this, _, window, cx| this.run_action(window, cx)))
        .child(countdown(action.shortcut, progress, cx))
        .child(
            div()
                .type_role(TypeRole::Button, cx)
                .text_color(colors.text_accent)
                .child(action.label.clone()),
        )
        .children(beside)
}

/// The shortcut inside a ring that drains as the toast's time runs out.
fn countdown(shortcut: Shortcut, progress: f32, cx: &App) -> impl IntoElement {
    let colors = cx.theme().colors();
    let (track, fill) = (colors.text.opacity(0.12), colors.text_accent);
    let keys = shortcut.keys();
    let (diameter, key_size) = ring_metrics(&keys);
    div()
        .relative()
        .flex_none()
        .size(px(diameter))
        .flex()
        .items_center()
        .justify_center()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let center = bounds.center();
                    let radius = bounds.size.width.min(bounds.size.height) / 2. - px(1.5);
                    paint_arc(window, center, radius, 1., track);
                    paint_arc(window, center, radius, progress.clamp(0., 1.), fill);
                },
            )
            .absolute()
            .size_full(),
        )
        .children(key_size.map(|size| {
            div()
                .text_size(px(size))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(colors.text)
                .child(keys)
        }))
}

fn paint_arc(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    radius: Pixels,
    fraction: f32,
    color: Hsla,
) {
    if fraction <= 0. {
        return;
    }
    let segments = (48. * fraction).ceil().max(2.) as usize;
    let start = -std::f32::consts::FRAC_PI_2;
    let sweep = std::f32::consts::TAU * fraction;
    let at = |step: usize| {
        let angle = start + sweep * step as f32 / segments as f32;
        point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        )
    };
    let mut path = PathBuilder::stroke(px(2.));
    path.move_to(at(0));
    for step in 1..=segments {
        path.line_to(at(step));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn close(cx: &mut Context<ToastView>) -> impl IntoElement {
    let colors = cx.theme().colors();
    div()
        .id("toast-dismiss")
        .flex_none()
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(colors.text_muted)
        .hover(|style| style.bg(colors.element_hover))
        .cursor_pointer()
        .font_family("lucide")
        .text_size(px(13.))
        .child(char::from(Icon::X).to_string())
        .on_click(cx.listener(|this, _, _, cx| {
            this.dismiss(cx);
        }))
}

/// Drop in with a little overshoot; an error then shakes once.
pub(super) fn arrive(
    island: gpui::Stateful<gpui::Div>,
    kind: ToastKind,
    generation: u64,
) -> AnyElement {
    let shake = kind == ToastKind::Error;
    let total = if shake { 800 } else { 420 };
    let drop_share = 420. / total as f32;
    island
        .with_animation(
            ElementId::NamedInteger("toast-arrive".into(), generation),
            Animation::new(Duration::from_millis(total)),
            move |island, t| {
                let drop = back_out((t / drop_share).min(1.));
                let offset = if shake && t > drop_share {
                    shake_offset((t - drop_share) / (1. - drop_share))
                } else {
                    0.
                };
                island
                    .mt(px(-18. * (1. - drop)))
                    .ml(px(offset))
                    .opacity((t / drop_share * 2.).min(1.))
            },
        )
        .into_any_element()
}

fn back_out(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.;
    1. + c3 * (t - 1.).powi(3) + c1 * (t - 1.).powi(2)
}

fn shake_offset(t: f32) -> f32 {
    4. * (1. - t) * (t * std::f32::consts::TAU * 2.).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overshoot_settles_at_rest() {
        assert!((back_out(1.) - 1.).abs() < 1e-5);
        assert!(back_out(0.7) > 1.);
        assert!(shake_offset(1.).abs() < 1e-5);
    }

    #[test]
    fn long_shortcuts_get_a_wider_ring() {
        assert!(ring_metrics("⌘⌥K").0 > ring_metrics("⌘Z").0);
    }
}
