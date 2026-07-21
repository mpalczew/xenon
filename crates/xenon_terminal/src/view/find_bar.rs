//! VS Code-style find strip chrome for the terminal (mirrors editor find bar).

use gpui::{
    App, AppContext, Context, ElementInputHandler, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div,
    px,
};
use theme::ActiveTheme;

use super::TerminalView;
use super::find_session::FindToggle;

#[derive(Clone, Copy)]
enum FindAction {
    Next,
    Prev,
    Close,
}

struct ChipTooltip {
    text: SharedString,
}

impl Render for ChipTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

impl TerminalView {
    pub(super) fn render_find_bar(
        &self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let find = self.find.as_ref()?;
        if !find.open {
            return None;
        }
        let (query_label, query_color) = if find.query.is_empty() {
            ("Find".to_string(), colors.text_muted)
        } else {
            (find.query.clone(), colors.text)
        };
        let status = self.find_status_label();
        let opts = find.options;
        let focus = find.focus.clone();
        let entity = cx.entity();

        Some(
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
                .track_focus(&focus)
                .key_context("TerminalFind")
                .on_key_down(cx.listener(Self::on_find_key))
                .child(
                    div()
                        .id("term-find-query")
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.terminal_background)
                        .text_sm()
                        .text_color(query_color)
                        .child(query_label)
                        .child(find_input_canvas(entity, focus)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .min_w(px(72.))
                        .child(status),
                )
                .children(find_control_chips(opts, colors, cx)),
        )
    }

    pub(super) fn on_find_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        if mods.alt && !mods.platform && !mods.control {
            match key {
                "c" | "C" => {
                    self.toggle_find_option(FindToggle::Case, cx);
                    cx.stop_propagation();
                    return;
                }
                "w" | "W" => {
                    self.toggle_find_option(FindToggle::Word, cx);
                    cx.stop_propagation();
                    return;
                }
                "r" | "R" => {
                    self.toggle_find_option(FindToggle::Regex, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        match key {
            "escape" => {
                self.close_find(window, cx);
                cx.stop_propagation();
            }
            "enter" => {
                if mods.shift {
                    self.find_previous(cx);
                } else {
                    self.find_next(cx);
                }
                cx.stop_propagation();
            }
            "backspace" => {
                self.find_backspace(cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }
}

fn find_input_canvas(view: gpui::Entity<TerminalView>, focus: FocusHandle) -> impl IntoElement {
    canvas(
        move |_bounds, _window, _cx| {},
        move |bounds, _prepaint, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

fn chip_tooltip(tip: SharedString) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    move |_window: &mut Window, cx: &mut App| cx.new(|_| ChipTooltip { text: tip.clone() }).into()
}

struct ToggleChip {
    label: &'static str,
    tip: &'static str,
    active: bool,
    which: FindToggle,
}

fn find_control_chips(
    opts: crate::find::FindOptions,
    colors: &theme::ThemeColors,
    cx: &mut Context<TerminalView>,
) -> Vec<gpui::AnyElement> {
    vec![
        toggle_btn(
            ToggleChip {
                label: "Aa",
                tip: "Match case · ⌥C",
                active: opts.case_sensitive,
                which: FindToggle::Case,
            },
            colors,
            cx,
        )
        .into_any_element(),
        toggle_btn(
            ToggleChip {
                label: "W",
                tip: "Match whole word · ⌥W",
                active: opts.whole_word,
                which: FindToggle::Word,
            },
            colors,
            cx,
        )
        .into_any_element(),
        toggle_btn(
            ToggleChip {
                label: ".*",
                tip: "Use regular expression · ⌥R",
                active: opts.regex,
                which: FindToggle::Regex,
            },
            colors,
            cx,
        )
        .into_any_element(),
        action_btn(
            "↑",
            "Previous match · ⇧↵ / ⌘⇧G",
            colors,
            cx,
            FindAction::Prev,
        )
        .into_any_element(),
        action_btn("↓", "Next match · ↵ / ⌘G", colors, cx, FindAction::Next).into_any_element(),
        action_btn("✕", "Close · Esc", colors, cx, FindAction::Close).into_any_element(),
    ]
}

fn toggle_btn(
    chip: ToggleChip,
    colors: &theme::ThemeColors,
    cx: &mut Context<TerminalView>,
) -> impl IntoElement + use<> {
    let bg = if chip.active {
        colors.element_selected
    } else {
        colors.ghost_element_background
    };
    let fg = if chip.active {
        colors.text
    } else {
        colors.text_muted
    };
    let tip = SharedString::from(chip.tip);
    let which = chip.which;
    div()
        .id(chip.label)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .bg(bg)
        .text_xs()
        .text_color(fg)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
        .tooltip(chip_tooltip(tip))
        .child(chip.label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_find_option(which, cx);
        }))
}

fn action_btn(
    label: &'static str,
    tip: &'static str,
    colors: &theme::ThemeColors,
    cx: &mut Context<TerminalView>,
    action: FindAction,
) -> impl IntoElement + use<> {
    let tip = SharedString::from(tip);
    div()
        .id(label)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .bg(colors.ghost_element_background)
        .text_xs()
        .text_color(colors.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
        .tooltip(chip_tooltip(tip))
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| match action {
            FindAction::Next => this.find_next(cx),
            FindAction::Prev => this.find_previous(cx),
            FindAction::Close => this.close_find(window, cx),
        }))
}
