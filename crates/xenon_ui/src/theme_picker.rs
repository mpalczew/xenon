//! Theme gallery overlay (⌘⌥T): labeled cards with a mini chrome preview.
//! Separate from Settings. Type to filter, arrows, Return, Escape.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use nucleo::{Config, Matcher};
use theme::{ActiveTheme, Appearance, Theme, ThemeColors, ThemeRegistry};
use xenon_store::ThemeMode;

use crate::impl_palette_query_input;
use crate::palette::{
    PaletteLayout, fuzzy_index_order, hint_row, input_registrar, optional_title, panel, query_row,
    scrim,
};

const COLS: usize = 3;
const CARD_PREVIEW_H: f32 = 88.;

pub enum ThemePickerEvent {
    Dismissed,
}

struct ThemeItem {
    name: String,
    appearance: Appearance,
}

pub struct ThemePickerView {
    items: Vec<ThemeItem>,
    query: String,
    results: Vec<usize>,
    selected: usize,
    current: String,
    focus: FocusHandle,
    focused_once: bool,
    matcher: Matcher,
    scroll: ScrollHandle,
}

impl EventEmitter<ThemePickerEvent> for ThemePickerView {}

impl ThemePickerView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let snap = xenon_settings::snapshot(cx);
        let current = match snap.theme {
            ThemeMode::Light => snap.light_theme.clone(),
            ThemeMode::Dark => snap.dark_theme.clone(),
            ThemeMode::System => match theme::SystemAppearance::global(cx).0 {
                Appearance::Light => snap.light_theme.clone(),
                Appearance::Dark => snap.dark_theme.clone(),
            },
        };
        let mut items: Vec<ThemeItem> = ThemeRegistry::global(cx)
            .list()
            .into_iter()
            .map(|meta| ThemeItem {
                name: meta.name.to_string(),
                appearance: meta.appearance,
            })
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        let mut view = Self {
            items,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            current,
            focus: cx.focus_handle(),
            focused_once: false,
            matcher: Matcher::new(Config::DEFAULT),
            scroll: ScrollHandle::new(),
        };
        view.refilter();
        if let Some(i) = view
            .results
            .iter()
            .position(|&idx| view.items[idx].name == view.current)
        {
            view.selected = i;
        }
        view
    }

    fn refilter(&mut self) {
        let haystacks: Vec<String> = self
            .items
            .iter()
            .map(|item| format!("{} {:?}", item.name, item.appearance))
            .collect();
        self.results = fuzzy_index_order(&haystacks, &self.query, &mut self.matcher);
        self.selected = 0;
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.refilter();
        cx.notify();
    }

    fn move_grid(&mut self, key: &str, cx: &mut Context<Self>) {
        self.selected = step_grid(self.selected, self.results.len(), COLS, key);
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(&idx) = self.results.get(self.selected) else {
            return;
        };
        let name = self.items[idx].name.clone();
        let appearance = self.items[idx].appearance;
        apply_named_theme(&name, appearance, cx);
        self.current = name;
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => cx.emit(ThemePickerEvent::Dismissed),
            "enter" => self.confirm(cx),
            "left" | "up" | "right" | "down" => self.move_grid(event.keystroke.key.as_str(), cx),
            "backspace" => {
                let mut q = self.query.clone();
                q.pop();
                self.set_query(q, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// Arrow motion on a wrapping card grid.
pub(crate) fn step_grid(selected: usize, len: usize, cols: usize, key: &str) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    let cols = cols.max(1);
    match key {
        "left" => selected.saturating_sub(1),
        "right" => (selected + 1).min(last),
        "up" => selected.saturating_sub(cols),
        "down" => (selected + cols).min(last),
        _ => selected,
    }
}

fn preview_card(theme: &Theme, current: bool, selected: bool) -> impl IntoElement + use<> {
    let colors = theme.colors();
    let ring = if selected {
        colors.border_focused
    } else if current {
        colors.text_accent
    } else {
        colors.border
    };
    div()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(ring)
        .overflow_hidden()
        .child(chrome_demo(colors))
}

fn chrome_demo(colors: &ThemeColors) -> impl IntoElement + use<> {
    div()
        .h(px(CARD_PREVIEW_H))
        .flex()
        .flex_col()
        .bg(colors.background)
        .child(
            div()
                .h(px(10.))
                .flex_none()
                .bg(colors.title_bar_background)
                .border_b_1()
                .border_color(colors.border),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .w(px(28.))
                        .flex_none()
                        .h_full()
                        .bg(colors.panel_background)
                        .border_r_1()
                        .border_color(colors.border)
                        .child(
                            div()
                                .mt(px(4.))
                                .mx(px(4.))
                                .h(px(6.))
                                .rounded_sm()
                                .bg(colors.element_selected),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .h(px(12.))
                                .flex_none()
                                .bg(colors.tab_bar_background)
                                .border_b_1()
                                .border_color(colors.border)
                                .child(
                                    div()
                                        .w(px(36.))
                                        .h_full()
                                        .bg(colors.tab_active_background)
                                        .border_b_1()
                                        .border_color(colors.text_accent),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .bg(colors.terminal_background)
                                .px(px(6.))
                                .py_1()
                                .child(div().text_xs().text_color(colors.text).child("xenon"))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.text_accent)
                                        .child("ready"),
                                ),
                        ),
                ),
        )
}

impl Focusable for ThemePickerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ThemePickerView {
    fn cards(&self, colors: &ThemeColors, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let registry = ThemeRegistry::global(cx);
        self.results
            .iter()
            .enumerate()
            .filter_map(|(i, &item_i)| {
                let item = self.items.get(item_i)?;
                let theme = registry.get(&item.name).ok()?;
                let selected = i == self.selected;
                let current = item.name == self.current;
                let name = item.name.clone();
                let kind = match item.appearance {
                    Appearance::Light => "Light",
                    Appearance::Dark => "Dark",
                };
                let caption = if current {
                    format!("{kind} · current")
                } else {
                    kind.to_string()
                };
                Some(
                    div()
                        .id(("theme-card", i))
                        .w(px(236.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = i;
                            this.confirm(cx);
                        }))
                        .child(preview_card(&theme, current, selected))
                        .child(
                            div()
                                .px(px(2.))
                                .text_sm()
                                .font_weight(if selected || current {
                                    gpui::FontWeight::MEDIUM
                                } else {
                                    gpui::FontWeight::NORMAL
                                })
                                .text_color(if selected {
                                    colors.text
                                } else {
                                    colors.text_muted
                                })
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .px(px(2.))
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(caption),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }
}

impl Render for ThemePickerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors().clone();
        let layout = PaletteLayout {
            width: 780.,
            max_h: 560.,
            top: 48.,
        };
        let cards = self.cards(&colors, cx);

        scrim("theme-picker-scrim", layout)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ThemePickerEvent::Dismissed)))
            .child(
                panel(layout, &colors)
                    .track_focus(&self.focus)
                    .key_context("ThemePicker")
                    .on_key_down(cx.listener(Self::on_key))
                    .child(input_registrar(cx.entity(), self.focus.clone()).into_any_element())
                    .child(optional_title("Themes", &colors).into_any_element())
                    .child(
                        query_row(&self.query, "Filter themes…", true, &colors).into_any_element(),
                    )
                    .child(
                        div()
                            .id("theme-picker-grid")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .p(px(12.))
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_3()
                            .children(cards),
                    )
                    .child(
                        hint_row(
                            "↵ apply (stays open)  ·  esc dismiss  ·  arrows move  ·  type to filter",
                            &colors,
                        )
                        .into_any_element(),
                    ),
            )
    }
}

fn apply_named_theme(name: &str, appearance: Appearance, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    match appearance {
        Appearance::Light => {
            settings.light_theme = name.to_string();
            settings.theme = ThemeMode::Light;
        }
        Appearance::Dark => {
            settings.dark_theme = name.to_string();
            settings.theme = ThemeMode::Dark;
        }
    }
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::apply_theme(cx);
}

impl_palette_query_input!(ThemePickerView);

#[cfg(test)]
mod tests {
    use super::step_grid;

    #[test]
    fn grid_arrows() {
        assert_eq!(step_grid(4, 9, 3, "left"), 3);
        assert_eq!(step_grid(4, 9, 3, "right"), 5);
        assert_eq!(step_grid(4, 9, 3, "up"), 1);
        assert_eq!(step_grid(4, 9, 3, "down"), 7);
        assert_eq!(step_grid(0, 9, 3, "left"), 0);
        assert_eq!(step_grid(8, 9, 3, "right"), 8);
        assert_eq!(step_grid(7, 9, 3, "down"), 8);
    }
}
