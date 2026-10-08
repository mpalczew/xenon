//! Theme side panel (⌘⌥T): one row per family with dark and light snippets.
//! Moving the selection re-themes the whole app live; Return keeps it in its
//! slot, Escape puts back what was showing. No scrim: the app is the preview.

mod families;
mod tile;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use theme::{ActiveTheme, Appearance};
use xenon_design_system::{ActionButton, FocusOnOpen, TypeRole, Typography, action_button};
use xenon_store::ThemeMode;

use families::{Family, group_title, lineup};
use tile::{TileState, tile};

pub(crate) const PANEL_WIDTH: f32 = 340.;

pub enum ThemePickerEvent {
    Dismissed,
}

pub struct ThemePickerView {
    families: Vec<Family>,
    selected: usize,
    half: Appearance,
    focus: FocusHandle,
    focus_on_open: FocusOnOpen,
    scroll: ScrollHandle,
    /// Scroll the selection into view once the rows have been laid out.
    follow: bool,
}

impl EventEmitter<ThemePickerEvent> for ThemePickerView {}

impl ThemePickerView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let families = lineup(cx);
        let half = showing_appearance(cx);
        let live = live_theme_name(cx);
        let selected = families.iter().position(|f| f.contains(&live)).unwrap_or(0);
        let focus = cx.focus_handle();
        let mut focus_on_open = FocusOnOpen::new(focus.clone());
        focus_on_open.open();
        Self {
            families,
            selected,
            half,
            focus,
            focus_on_open,
            scroll: ScrollHandle::new(),
            follow: true,
        }
    }

    fn select(&mut self, index: usize, half: Appearance, cx: &mut Context<Self>) {
        let Some(family) = self.families.get(index) else {
            return;
        };
        let (half, name) = family.nearest(half);
        self.selected = index;
        self.half = half;
        xenon_terminal::preview_theme(name, cx);
        self.follow = true;
        cx.notify();
    }

    /// Store the selection in its slot, then show the Mode's theme again.
    fn keep(&mut self, cx: &mut Context<Self>) {
        if let Some(family) = self.families.get(self.selected) {
            let (half, name) = family.nearest(self.half);
            crate::app::apply_named_theme(name, half, cx);
        }
        xenon_terminal::apply_theme(cx);
        cx.emit(ThemePickerEvent::Dismissed);
    }

    fn revert(&mut self, cx: &mut Context<Self>) {
        xenon_terminal::apply_theme(cx);
        cx.emit(ThemePickerEvent::Dismissed);
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let last = self.families.len().saturating_sub(1);
        match event.keystroke.key.as_str() {
            "up" => self.select(self.selected.saturating_sub(1), self.half, cx),
            "down" => self.select((self.selected + 1).min(last), self.half, cx),
            "left" => self.select(self.selected, Appearance::Dark, cx),
            "right" => self.select(self.selected, Appearance::Light, cx),
            "enter" => self.keep(cx),
            "escape" => self.revert(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn follow_selection(&mut self, cx: &mut Context<Self>) {
        if !self.follow {
            return;
        }
        let child = self.child_index(self.selected);
        self.scroll.scroll_to_item(child);
        if self.scroll.bounds_for_item(child).is_some() {
            self.follow = false;
        } else {
            cx.notify();
        }
    }

    /// Scroll children are group titles and rows, in order.
    fn child_index(&self, row: usize) -> usize {
        let mut groups = 0;
        let mut last = "";
        for family in &self.families[..=row.min(self.families.len().saturating_sub(1))] {
            if family.group != last {
                groups += 1;
                last = family.group;
            }
        }
        row + groups
    }
}

#[cfg(feature = "visual-tests")]
impl ThemePickerView {
    pub fn visual_select(&mut self, index: usize, half: Appearance, cx: &mut Context<Self>) {
        self.select(index, half, cx);
    }
}

impl Focusable for ThemePickerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ThemePickerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focus_on_open.focus_after_open(window, cx);
        self.follow_selection(cx);
        let colors = cx.theme().colors().clone();
        let saved = saved_names(cx);
        let mut children = Vec::new();
        let mut last = "";
        for (index, family) in self.families.iter().enumerate() {
            if family.group != last {
                last = family.group;
                children.push(group_label(family.group, cx).into_any_element());
            }
            let in_use = saved.iter().any(|name| family.contains(name));
            children.push(self.row(index, family, in_use, cx).into_any_element());
        }
        div()
            .id("theme-panel")
            .track_focus(&self.focus)
            .key_context("ThemePicker")
            .on_key_down(cx.listener(Self::on_key))
            .occlude()
            .w(px(PANEL_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.panel_background)
            .border_l_1()
            .border_color(colors.border)
            .shadow_lg()
            .child(header(cx))
            .child(
                div()
                    .id("theme-panel-list")
                    .track_scroll(&self.scroll)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .pb_3()
                    .children(children),
            )
            .child(footer(cx))
    }
}

impl ThemePickerView {
    fn row(
        &self,
        index: usize,
        family: &Family,
        in_use: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let on = index == self.selected;
        let tile_for = |half: Appearance, cx: &mut Context<Self>| {
            tile(
                family.theme(half),
                half,
                TileState {
                    selected: on && self.half == half,
                },
                cx,
            )
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                cx.stop_propagation();
                if event.click_count() > 1 {
                    this.keep(cx);
                } else {
                    this.select(index, half, cx);
                }
            }))
        };
        let dark = tile_for(Appearance::Dark, cx);
        let light = tile_for(Appearance::Light, cx);
        div()
            .id(SharedString::from(format!("theme-family-{}", family.name)))
            .mb_1()
            .p(px(6.))
            .rounded_md()
            .border_1()
            .border_color(if on {
                colors.text_accent.opacity(0.5)
            } else {
                gpui::transparent_black()
            })
            .bg(if on {
                colors.element_selected
            } else {
                gpui::transparent_black()
            })
            .hover(|s| s.bg(colors.element_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb(px(5.))
                    .px(px(2.))
                    .child(
                        div()
                            .type_role(TypeRole::ListPrimary, cx)
                            .text_color(if on { colors.text } else { colors.text_muted })
                            .child(family.name),
                    )
                    .children(in_use.then(|| in_use_pill(cx))),
            )
            .child(div().flex().gap_2().child(dark).child(light))
    }
}

fn header(cx: &mut Context<ThemePickerView>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(colors.border)
        .child(div().type_role(TypeRole::SectionTitle, cx).child("Themes"))
        .child(
            div()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child(xenon_design_system::shortcut_text("⌘⌥T")),
        )
}

fn group_label(group: &str, cx: &mut Context<ThemePickerView>) -> impl IntoElement + use<> {
    div()
        .pt_3()
        .pb_1()
        .px_1()
        .type_role(TypeRole::ControlLabel, cx)
        .child(group_title(group))
}

fn in_use_pill(cx: &mut Context<ThemePickerView>) -> impl IntoElement + use<> {
    let accent = cx.theme().colors().text_accent;
    div()
        .px(px(6.))
        .rounded_full()
        .border_1()
        .border_color(accent.opacity(0.45))
        .type_role(TypeRole::ControlLabel, cx)
        .text_color(accent)
        .child("in use")
}

fn footer(cx: &mut Context<ThemePickerView>) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .border_t_1()
        .border_color(colors.border)
        .child(
            div()
                .flex_1()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child("↑↓ theme · ←→ dark/light"),
        )
        .child(action_button(
            "theme-revert",
            ActionButton::quiet("Revert  esc"),
            cx,
            cx.listener(|this, _, _, cx| this.revert(cx)),
        ))
        .child(action_button(
            "theme-keep",
            ActionButton::primary("Keep  ↵"),
            cx,
            cx.listener(|this, _, _, cx| this.keep(cx)),
        ))
}

fn showing_appearance(cx: &App) -> Appearance {
    match xenon_settings::snapshot(cx).theme {
        ThemeMode::Light => Appearance::Light,
        ThemeMode::Dark => Appearance::Dark,
        ThemeMode::System => theme::SystemAppearance::global(cx).0,
    }
}

fn live_theme_name(cx: &App) -> String {
    let settings = xenon_settings::snapshot(cx);
    match showing_appearance(cx) {
        Appearance::Light => settings.light_theme,
        Appearance::Dark => settings.dark_theme,
    }
}

fn saved_names(cx: &App) -> [String; 2] {
    let settings = xenon_settings::snapshot(cx);
    [settings.dark_theme, settings.light_theme]
}
