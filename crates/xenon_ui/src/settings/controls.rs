//! Settings-only controls: segmented choice and the theme swatch strip.

use gpui::{
    BoxShadow, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, point, prelude::FluentBuilder, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

use super::SettingsView;
use super::appearance::pick_swatch;
use super::row::{Pick, Swatch};

pub(super) struct Choice<'a> {
    pub id: &'a SharedString,
    pub options: &'a [SharedString],
    pub selected: usize,
    pub pick: &'a Pick,
}

/// Segmented control: the selected segment carries fill, weight, and text color.
pub(super) fn choice(
    Choice {
        id,
        options,
        selected,
        pick,
    }: Choice<'_>,
    row: usize,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let segments = options.iter().enumerate().map(|(index, label)| {
        let on = index == selected;
        let pick = pick.clone();
        div()
            .id(SharedString::from(format!("{id}-{index}")))
            .px(px(11.))
            .py(px(3.))
            .rounded_sm()
            .type_role(TypeRole::Button, cx)
            .font_weight(if on {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(if on { colors.text } else { colors.text_muted })
            .when(on, |segment| segment.bg(colors.element_selected))
            .cursor_pointer()
            .hover(|s| s.text_color(colors.text))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                pick(index, window, cx);
                this.focus_row(row, window, cx);
            }))
            .child(label.clone())
    });
    div()
        .flex()
        .flex_none()
        .p(px(2.))
        .gap(px(2.))
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(colors.editor_background)
        .children(segments)
}

/// Theme cards: editor background, accent, and keyword color, with the name.
pub(super) fn swatch_strip(
    swatches: &[Swatch],
    row: usize,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let cards = swatches.iter().map(|swatch| {
        let pick = swatch.clone();
        let ring = if swatch.current {
            colors.text_accent
        } else {
            colors.border
        };
        let mut card = div()
            .id(SharedString::from(format!("swatch-{}", swatch.name)))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded_md()
            .border_1()
            .border_color(ring)
            .overflow_hidden()
            .bg(colors.editor_background)
            .cursor_pointer()
            .hover(|s| s.border_color(colors.border_focused))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                pick_swatch(&pick, cx);
                this.focus_row(row, window, cx);
            }))
            .child(
                div()
                    .h(px(34.))
                    .flex()
                    .children(swatch.colors.map(|color| div().flex_1().bg(color))),
            )
            .child(
                div()
                    .px(px(6.))
                    .py(px(3.))
                    .type_role(TypeRole::ControlLabel, cx)
                    .font_weight(if swatch.current {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if swatch.current {
                        colors.text
                    } else {
                        colors.text_muted
                    })
                    .truncate()
                    .child(swatch.name.clone()),
            );
        if swatch.current {
            card = card.shadow(vec![BoxShadow {
                color: colors.text_accent.opacity(0.25),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(2.),
                inset: false,
            }]);
        }
        card
    });
    div().flex().gap_2().children(cards)
}
