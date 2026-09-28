//! The title row (with its optional checkbox) and bullet rows of an outline.

use gpui::{Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use theme::ActiveTheme;

use super::{OutlineEvent, OutlineView};
use crate::text_input::TextInputView;
use crate::typography::{TypeRole, Typography};

pub(super) fn title_line(
    checked: Option<bool>,
    input: Entity<TextInputView>,
    spaced: bool,
    cx: &mut Context<OutlineView>,
) -> impl IntoElement {
    let row = div().flex().items_center().gap_2().min_h(px(28.));
    let row = if spaced { row.mb_3() } else { row };
    row.type_role(TypeRole::ListPrimary, cx)
        .children(checked.map(|checked| {
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(crate::checkbox(
                    "outline-title-check",
                    crate::CheckboxState {
                        checked,
                        disabled: false,
                    },
                    "Complete item",
                    cx,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.title_check = Some(!checked);
                        cx.emit(OutlineEvent::Checked(!checked));
                        cx.notify();
                    }),
                ))
        }))
        .child(div().flex_1().min_w_0().child(input))
}

pub(super) fn point_line(
    depth: u8,
    input: Entity<TextInputView>,
    cx: &mut Context<OutlineView>,
) -> impl IntoElement {
    let color = cx.theme().colors().text_accent;
    let size = if depth == 0 { px(5.) } else { px(4.) };
    div()
        .flex()
        .items_center()
        .gap_2()
        .h(px(28.))
        .pl(px(f32::from(depth) * 22.))
        .type_role(TypeRole::Body, cx)
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(size).h(size).rounded_full().bg(color)),
        )
        .child(div().flex_1().min_w_0().child(input))
}
