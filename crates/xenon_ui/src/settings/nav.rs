//! Settings sidebar: search field and the page list.

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_design_system::{TextInputView, TypeRole, Typography, selectable_row};

use super::SettingsView;
use super::page::SettingsPage;
use crate::icons::icon;

pub(super) const SIDEBAR_WIDTH: f32 = 200.;

/// `selected` is None while search results replace the page.
pub(super) fn sidebar(
    selected: Option<SettingsPage>,
    search: &Entity<TextInputView>,
    remote_on: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let searching = selected.is_none();
    let items = SettingsPage::ALL
        .iter()
        .enumerate()
        .map(|(index, &page)| {
            nav_item(page, index, selected == Some(page), remote_on, cx).into_any_element()
        })
        .collect::<Vec<_>>();
    div()
        .w(px(SIDEBAR_WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(2.))
        .px_2()
        .py_3()
        .bg(linear_gradient(
            180.,
            linear_color_stop(colors.panel_background, 0.),
            linear_color_stop(colors.background, 1.),
        ))
        .border_r_1()
        .border_color(colors.border)
        .child(search_field(search, searching, cx))
        .children(items)
}

fn search_field(
    search: &Entity<TextInputView>,
    searching: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div()
        .id("settings-search")
        .mb_2()
        .px_2()
        .py(px(6.))
        .flex()
        .items_center()
        .gap_2()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(colors.editor_background)
        .text_color(colors.text_muted)
        .on_click(cx.listener(|this, _, window, cx| this.focus_search(window, cx)))
        .child(icon(Icon::Search, px(13.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .type_role(TypeRole::ControlLabel, cx)
                .child(search.clone()),
        )
        .children((!searching).then(|| {
            div()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child(xenon_design_system::shortcut_text("⌘F"))
        }))
}

fn nav_item(
    page: SettingsPage,
    index: usize,
    selected: bool,
    remote_on: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let badge = (page == SettingsPage::PhoneRemote && remote_on).then(|| {
        div()
            .px(px(6.))
            .rounded_full()
            .bg(colors.text_accent.opacity(0.16))
            .type_role(TypeRole::ControlLabel, cx)
            .text_color(colors.text_accent)
            .child("On")
    });
    selectable_row(
        SharedString::from(format!("settings-nav-{index}")),
        selected,
        &colors,
    )
    .py(px(6.))
    .child(
        div()
            .w(px(18.))
            .flex()
            .justify_center()
            .text_color(if selected {
                colors.text_accent
            } else {
                colors.text_muted
            })
            .child(icon(page.icon(), px(14.))),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .type_role(TypeRole::Body, cx)
            .text_color(if selected {
                colors.text
            } else {
                colors.text_muted
            })
            .when(selected, |title| {
                title.font_weight(gpui::FontWeight::SEMIBOLD)
            })
            .child(page.title()),
    )
    .children(badge)
    .on_click(cx.listener(move |this, _, window, cx| this.show_page(page, window, cx)))
}
