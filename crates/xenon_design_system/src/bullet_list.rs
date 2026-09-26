//! Indented supporting text. Depth 0 is a disc; deeper lines are circles.

use gpui::{App, IntoElement, ParentElement, SharedString, Styled, div, px};

use crate::typography::{TypeRole, Typography};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulletLine {
    pub depth: u8,
    pub text: SharedString,
}

pub fn bullet_list(lines: Vec<BulletLine>, cx: &App) -> impl IntoElement {
    div().children(lines.into_iter().map(|line| {
        let mark = if line.depth == 0 { "•" } else { "◦" };
        div()
            .pl(px(f32::from(line.depth) * 22.))
            .type_role(TypeRole::Supporting, cx)
            .child(format!("{mark} {}", line.text))
    }))
}
