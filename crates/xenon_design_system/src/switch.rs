//! On/off switch for settings that apply immediately (checkboxes pick items).

use std::{sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, Hsla, InteractiveElement,
    IntoElement, ParentElement, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
    white,
};
use theme::ActiveTheme;

const TRACK_W: f32 = 30.;
const TRACK_H: f32 = 18.;
const KNOB: f32 = 14.;
const INSET: f32 = 2.;
const TRAVEL: f32 = TRACK_W - KNOB - 2. * INSET;

#[derive(Clone, Copy)]
pub struct SwitchState {
    pub on: bool,
    pub disabled: bool,
}

pub fn switch(
    id: impl Into<ElementId>,
    state: SwitchState,
    label: impl Into<gpui::SharedString>,
    cx: &App,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let id = id.into();
    let colors = cx.theme().colors();
    let off_track = colors.border;
    let on_track = colors.text_accent;
    let animate = !state.disabled && !super::motion_frozen(cx) && !super::reduce_motion();
    let control = div()
        .id(id.clone())
        .relative()
        .w(px(TRACK_W))
        .h(px(TRACK_H))
        .flex_none()
        .rounded_full()
        .bg(if state.on { on_track } else { off_track })
        .aria_label(label)
        .focusable()
        .child(knob(&id, state.on, animate, [off_track, on_track]));
    if state.disabled {
        control.opacity(0.45)
    } else {
        control.cursor_pointer().on_click(on_toggle)
    }
}

fn knob(id: &ElementId, on: bool, animate: bool, tracks: [Hsla; 2]) -> AnyElement {
    let rest = if on { TRAVEL } else { 0. };
    let face = div()
        .absolute()
        .top(px(INSET))
        .left(px(INSET + rest))
        .size(px(KNOB))
        .rounded_full()
        .bg(white())
        .shadow_sm();
    if !animate {
        return face.into_any_element();
    }
    let phase = if on { "on" } else { "off" };
    let [off_track, on_track] = tracks;
    let (from, to) = if on {
        (off_track, on_track)
    } else {
        (on_track, off_track)
    };
    let start = if on { 0. } else { TRAVEL };
    let face = face.with_animation(
        child_id(id, phase, "knob"),
        Animation::new(Duration::from_millis(180)),
        move |face, t| face.left(px(INSET + start + (rest - start) * settle(t))),
    );
    // The track color crossfades under the knob.
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded_full()
        .child(face)
        .with_animation(
            child_id(id, phase, "track"),
            Animation::new(Duration::from_millis(180)),
            move |track, t| track.bg(from.blend(to.opacity(t))),
        )
        .into_any_element()
}

fn child_id(id: &ElementId, phase: &'static str, part: &'static str) -> ElementId {
    let phase = ElementId::NamedChild(Arc::new(id.clone()), phase.into());
    ElementId::NamedChild(Arc::new(phase), part.into())
}

/// Ease-out with a small overshoot, settling at 1.
fn settle(t: f32) -> f32 {
    let s = 1.6;
    let u = t - 1.;
    1. + u * u * ((s + 1.) * u + s)
}

#[cfg(test)]
mod tests {
    use super::settle;

    #[test]
    fn switch_motion_overshoots_then_settles() {
        assert!(settle(0.).abs() < 0.001);
        assert!(settle(0.7) > 1.);
        assert!((settle(1.) - 1.).abs() < 0.001);
    }
}
