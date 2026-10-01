//! Indented supporting text. Depth 0 is a disc; deeper lines are circles.

use gpui::{App, IntoElement, ParentElement, Styled, div, px};

use crate::marked_text::{MarkedText, marked_text};
use crate::typography::{TypeRole, Typography};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulletLine {
    pub depth: u8,
    pub text: MarkedText,
}

pub fn bullet_list(lines: Vec<BulletLine>, cx: &App) -> impl IntoElement {
    let size = 14. * xenon_settings::ui_font(cx).size / 14.;
    let line_height = px(size * 1.25);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(lines.into_iter().map(|line| {
            let mark = if line.depth == 0 { "•" } else { "◦" };
            div()
                .flex()
                .items_start()
                .gap_2()
                .pl(px(f32::from(line.depth) * 22.))
                .child(
                    div()
                        .w(px(16.))
                        .h(line_height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_none()
                        .child(mark),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .type_role(TypeRole::Supporting, cx)
                        .line_height(line_height)
                        .child(marked_text(line.text, cx)),
                )
        }))
}
