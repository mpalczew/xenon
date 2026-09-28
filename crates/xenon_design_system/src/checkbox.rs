//! Shared task checkbox, including pointer, keyboard, and state-change motion.

use std::{sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, InteractiveElement,
    IntoElement, ParentElement, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
    white,
};
use lucide_icons::Icon;
use theme::{ActiveTheme, ThemeColors};

#[derive(Clone, Copy)]
pub struct CheckboxState {
    pub checked: bool,
    pub disabled: bool,
}

pub fn checkbox(
    id: impl Into<ElementId>,
    state: CheckboxState,
    label: impl Into<gpui::SharedString>,
    cx: &App,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let id = id.into();
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    let animate = !state.disabled && !super::motion_frozen(cx) && !super::reduce_motion();
    let control = div()
        .id(id.clone())
        .relative()
        .w(px(18.))
        .h(px(18.))
        .flex_none()
        .aria_label(label)
        .focusable()
        .children((state.checked && animate).then(|| check_ring(&id, accent)))
        .child(check_face(&id, state.checked, colors, animate));
    if state.disabled {
        control.opacity(0.45)
    } else {
        control.cursor_pointer().on_click(on_toggle)
    }
}

fn check_face(id: &ElementId, checked: bool, colors: &ThemeColors, animate: bool) -> AnyElement {
    let accent = colors.text_accent;
    let surface = colors.editor_background;
    let border = if checked { accent } else { colors.text_muted };
    let face = div()
        .absolute()
        .top_0()
        .left_0()
        .size(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(border)
        .bg(if checked { accent } else { surface })
        .text_color(white())
        .children(checked.then(|| check_mark(id, animate)));
    if !animate {
        return face.into_any_element();
    }
    let phase = if checked { "checking" } else { "unchecking" };
    let duration = if checked { 440 } else { 280 };
    face.with_animation(
        child_id(id, phase),
        Animation::new(Duration::from_millis(duration)),
        move |face, t| {
            let scale = pop_scale(t, checked);
            let side = 18. * scale;
            let inset = (18. - side) / 2.;
            let fill = if checked {
                surface.blend(accent.opacity(t))
            } else {
                accent.blend(surface.opacity(t))
            };
            face.size(px(side)).top(px(inset)).left(px(inset)).bg(fill)
        },
    )
    .into_any_element()
}

fn check_mark(id: &ElementId, animate: bool) -> AnyElement {
    let mark = div()
        .font_family("lucide")
        .text_size(px(14.))
        .line_height(px(14.))
        .child(char::from(Icon::Check).to_string());
    if animate {
        mark.with_animation(
            child_id(id, "mark"),
            Animation::new(Duration::from_millis(360)),
            |mark, t| mark.opacity(t).mt(px(4. * (1. - t))),
        )
        .into_any_element()
    } else {
        mark.into_any_element()
    }
}

fn check_ring(id: &ElementId, accent: gpui::Hsla) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size(px(18.))
        .rounded_md()
        .bg(accent.opacity(0.2))
        .with_animation(
            child_id(id, "ring"),
            Animation::new(Duration::from_millis(440)),
            move |ring, t| {
                let side = 18. + 26. * t;
                let inset = (18. - side) / 2.;
                ring.size(px(side))
                    .top(px(inset))
                    .left(px(inset))
                    .bg(accent.opacity(0.2 * (1. - t)))
            },
        )
        .into_any_element()
}

fn child_id(id: &ElementId, name: &'static str) -> ElementId {
    ElementId::NamedChild(Arc::new(id.clone()), name.into())
}

fn pop_scale(t: f32, checking: bool) -> f32 {
    if checking {
        if t < 0.45 {
            0.76 + 0.47 * (t / 0.45)
        } else {
            1.23 - 0.23 * ((t - 0.45) / 0.55)
        }
    } else if t < 0.6 {
        1.12 - 0.24 * (t / 0.6)
    } else {
        0.88 + 0.12 * ((t - 0.6) / 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::pop_scale;

    #[test]
    fn checkbox_pop_settles_at_resting_size() {
        assert!(pop_scale(0.45, true) > 1.);
        assert!(pop_scale(0.6, false) < 1.);
        assert!((pop_scale(1., true) - 1.).abs() < 0.001);
        assert!((pop_scale(1., false) - 1.).abs() < 0.001);
    }
}
