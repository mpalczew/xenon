//! Settings keyboard model: ⌘1–⌘7 pages, ↑/↓ rows, ←/→ values,
//! Space/Enter activate, ⌘F or typing searches, Escape backs out.

use gpui::{Context, KeyDownEvent, Window};

use super::SettingsView;
use super::appearance::{open_theme_gallery, step_swatch};
use super::page::SettingsPage;
use super::row::{Control, visible_rows};

impl SettingsView {
    pub(super) fn on_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.handle_field_key(event, window, cx) {
            return;
        }
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let modifiers = keystroke.modifiers;
        if modifiers.secondary() {
            if let Some(page) = SettingsPage::from_digit(key) {
                self.show_page(page, window, cx);
                self.keyboard = true;
            } else if key == "f" {
                self.focus_search(window, cx);
            } else {
                return;
            }
            cx.stop_propagation();
            return;
        }
        // Keys a focused child input left alone (plain characters, space) bubble
        // here; they belong to that input, not to row navigation or type-to-search.
        if modifiers.control
            || modifiers.alt
            || modifiers.function
            || !self.focus.is_focused(window)
        {
            return;
        }
        if self.navigate(key, window, cx) {
            self.keyboard = true;
            cx.stop_propagation();
            return;
        }
        let typed = keystroke
            .key_char
            .as_deref()
            .filter(|c| c.chars().count() == 1 && c.chars().all(|ch| ch.is_alphanumeric()));
        if let Some(typed) = typed {
            self.start_search(typed, window, cx);
            cx.stop_propagation();
        }
    }

    /// Keys shared by the pane and the search field. Returns whether handled.
    pub(super) fn navigate(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match key {
            "up" => self.move_row(-1, cx),
            "down" => self.move_row(1, cx),
            "left" => self.nudge(false, window, cx),
            "right" => self.nudge(true, window, cx),
            "enter" | "space" | " " => self.activate(window, cx),
            "escape" => self.back_out(window, cx),
            _ => return false,
        }
        true
    }

    fn move_row(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = visible_rows(self.page, &self.query, cx).len();
        if count == 0 {
            return;
        }
        let next = (self.row as isize + delta).clamp(0, count as isize - 1);
        self.row = next as usize;
        self.scroll_to_row();
        cx.notify();
    }

    fn back_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.is_some() {
            self.dismiss_dropdown(window, cx);
        } else if !self.query.is_empty() {
            self.clear_search(window, cx);
        } else {
            window.remove_window();
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(located) = visible_rows(self.page, &self.query, cx)
            .into_iter()
            .nth(self.row)
        else {
            return;
        };
        match located.row.control {
            Control::Switch { toggle, .. } => toggle(window, cx),
            Control::Choice {
                options,
                selected,
                pick,
            } => pick((selected + 1) % options.len().max(1), window, cx),
            Control::Font { family, .. } => self.toggle_dropdown(family, window, cx),
            Control::Swatches(_) => open_theme_gallery(window, cx),
            Control::Field { field, value, .. } => self.begin_field_edit(field, value, window, cx),
            Control::Action(action) => (action.run)(window, cx),
            Control::None | Control::Value(_) => {
                // Search results jump to their page; plain rows have nothing to do.
                if !self.query.is_empty() {
                    self.reveal(located.page, located.index, window, cx);
                }
            }
        }
        window.refresh();
        cx.notify();
    }

    fn nudge(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(located) = visible_rows(self.page, &self.query, cx)
            .into_iter()
            .nth(self.row)
        else {
            return;
        };
        match located.row.control {
            Control::Choice {
                options,
                selected,
                pick,
            } => {
                let next = if forward {
                    (selected + 1).min(options.len().saturating_sub(1))
                } else {
                    selected.saturating_sub(1)
                };
                if next != selected {
                    pick(next, window, cx);
                }
            }
            Control::Font { size, .. } => {
                self.nudge_font_size(size, if forward { 1. } else { -1. }, cx)
            }
            Control::Swatches(swatches) => step_swatch(&swatches, forward, cx),
            _ => return,
        }
        window.refresh();
        cx.notify();
    }
}
