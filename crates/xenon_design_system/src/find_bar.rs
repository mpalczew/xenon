//! Shared in-pane find bar chrome and controls.

use gpui::{
    App, AppContext, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use theme::{ActiveTheme, ThemeColors};

use crate::TextInputView;

#[derive(Clone, Copy)]
pub struct FindBarOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Clone, Copy)]
pub enum FindBarAction {
    ToggleCase,
    ToggleWord,
    ToggleRegex,
    Previous,
    Next,
    Close,
}

pub struct FindBarConfig<'a, V> {
    pub input: Entity<TextInputView>,
    pub focus: FocusHandle,
    pub status: String,
    pub options: FindBarOptions,
    pub background: gpui::Hsla,
    pub key_context: &'static str,
    pub colors: &'a ThemeColors,
    pub on_key: fn(&mut V, &KeyDownEvent, &mut Window, &mut Context<V>),
    pub on_action: fn(&mut V, FindBarAction, &mut Window, &mut Context<V>),
}

fn find_query_field<V>(config: &FindBarConfig<'_, V>) -> impl IntoElement + use<V> {
    let focus = config.focus.clone();
    div()
        .id("find-query-field")
        .flex_1()
        .min_w_0()
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(config.colors.border)
        .bg(config.background)
        .text_sm()
        .child(config.input.clone())
        .on_click(move |_, window, cx| focus.focus(window, cx))
}

pub fn find_bar<V: 'static>(config: FindBarConfig<'_, V>, cx: &mut Context<V>) -> Div {
    let colors = config.colors;
    let options = config.options;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .track_focus(&config.focus)
        .key_context(config.key_context)
        .on_key_down(cx.listener(config.on_key))
        .child(find_query_field(&config))
        .child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .min_w(px(72.))
                .child(config.status.clone()),
        )
        .children([
            control(
                ControlSpec {
                    label: "Aa",
                    tip: "Match case · ⌥C",
                    active: options.case_sensitive,
                    action: FindBarAction::ToggleCase,
                },
                &config,
                cx,
            )
            .into_any_element(),
            control(
                ControlSpec {
                    label: "W",
                    tip: "Match whole word · ⌥W",
                    active: options.whole_word,
                    action: FindBarAction::ToggleWord,
                },
                &config,
                cx,
            )
            .into_any_element(),
            control(
                ControlSpec {
                    label: ".*",
                    tip: "Use regular expression · ⌥R",
                    active: options.regex,
                    action: FindBarAction::ToggleRegex,
                },
                &config,
                cx,
            )
            .into_any_element(),
            control(
                ControlSpec {
                    label: "↑",
                    tip: "Previous match · ⇧↵ / ⌘⇧G",
                    active: false,
                    action: FindBarAction::Previous,
                },
                &config,
                cx,
            )
            .into_any_element(),
            control(
                ControlSpec {
                    label: "↓",
                    tip: "Next match · ↵ / ⌘G",
                    active: false,
                    action: FindBarAction::Next,
                },
                &config,
                cx,
            )
            .into_any_element(),
            control(
                ControlSpec {
                    label: "✕",
                    tip: "Close · Esc",
                    active: false,
                    action: FindBarAction::Close,
                },
                &config,
                cx,
            )
            .into_any_element(),
        ])
}

struct ControlSpec {
    label: &'static str,
    tip: &'static str,
    active: bool,
    action: FindBarAction,
}

fn control<V: 'static>(
    spec: ControlSpec,
    config: &FindBarConfig<'_, V>,
    cx: &mut Context<V>,
) -> impl IntoElement + use<V> {
    let colors = config.colors;
    let bg = if spec.active {
        colors.element_selected
    } else {
        colors.ghost_element_background
    };
    let fg = if spec.active {
        colors.text
    } else {
        colors.text_muted
    };
    let tip = SharedString::from(spec.tip);
    let on_action = config.on_action;
    let action = spec.action;
    div()
        .id(spec.label)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .bg(bg)
        .text_xs()
        .text_color(fg)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
        .tooltip(move |_: &mut Window, cx: &mut App| {
            cx.new(|_| FindTooltip { text: tip.clone() }).into()
        })
        .child(spec.label)
        .on_click(cx.listener(move |this, _, window, cx| on_action(this, action, window, cx)))
}

struct FindTooltip {
    text: SharedString,
}

impl Render for FindTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}
