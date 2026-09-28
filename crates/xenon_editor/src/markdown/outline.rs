//! "On this page" outline beside the markdown preview's reading column.

use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography, selectable_row};

use super::nav::OutlineEntry;
use super::state::PreviewState;

/// Width of the outline column, including its padding.
pub(super) const OUTLINE_W: Pixels = px(220.);
/// Deeper headings are left out so the outline stays a table of contents.
const MAX_LEVEL: u8 = 3;
const INDENT: f32 = 12.;

/// Outline entries worth listing: two or more headings up to [`MAX_LEVEL`].
pub(super) fn entries(all: &[OutlineEntry]) -> Vec<OutlineEntry> {
    let shown: Vec<_> = all
        .iter()
        .filter(|entry| entry.level <= MAX_LEVEL)
        .cloned()
        .collect();
    if shown.len() < 2 { Vec::new() } else { shown }
}

/// Outline entries plus the one the reader is in.
pub(super) struct Outline {
    pub entries: Vec<OutlineEntry>,
    pub active: Option<usize>,
}

pub(super) fn panel(outline: &Outline, host: Entity<PreviewState>, cx: &App) -> AnyElement {
    let top = outline.entries.iter().map(|e| e.level).min().unwrap_or(1);
    let colors = cx.theme().colors();
    let rows = outline.entries.iter().enumerate().map(|(ix, entry)| {
        let selected = outline.active == Some(ix);
        let indent = f32::from(entry.level - top) * INDENT;
        let color = if selected {
            colors.text
        } else {
            colors.text_muted
        };
        let host = host.clone();
        selectable_row(("md-outline-row", ix), selected, colors)
            .py(px(4.))
            .pl(px(11. + indent))
            .min_w_0()
            .on_click(move |_, _, cx| {
                host.update(cx, |state, cx| state.jump_to_heading(ix, cx));
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(color)
                    .child(entry.title.clone()),
            )
    });
    div()
        .id("md-outline")
        .w(OUTLINE_W)
        .flex_none()
        .h_full()
        .min_h_0()
        .overflow_y_scroll()
        .pt(px(32.))
        .pb(px(24.))
        .pr(px(16.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .pl(px(13.))
                .pb(px(6.))
                .type_role(TypeRole::ControlLabel, cx)
                .child("On this page"),
        )
        .children(rows)
        .child(
            div()
                .pl(px(13.))
                .pt(px(10.))
                .type_role(TypeRole::ControlLabel, cx)
                .font_weight(gpui::FontWeight::NORMAL)
                .child("[ ]  previous / next heading"),
        )
        .into_any_element()
}
