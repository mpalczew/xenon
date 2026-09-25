//! Editor find bar assembled from the shared control.

use gpui::{Context, IntoElement, Window};
use xenon_design_system::{FindBarAction, FindBarConfig, FindBarOptions, find_bar};

use super::EditorView;
use super::find_session::FindToggle;

impl EditorView {
    pub(super) fn render_find_bar(
        &self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let find = self.find.as_ref()?;
        if !find.open {
            return None;
        }
        Some(find_bar(
            FindBarConfig {
                input: find.input.clone(),
                focus: find.focus.clone(),
                status: self.find_status_label(),
                options: FindBarOptions {
                    case_sensitive: find.options.case_sensitive,
                    whole_word: find.options.whole_word,
                    regex: find.options.regex,
                },
                background: colors.editor_background,
                key_context: "EditorFind",
                colors,
                on_key: Self::on_find_key,
                on_action: Self::on_find_action,
            },
            cx,
        ))
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
            let toggle = match key {
                "c" | "C" => Some(FindToggle::Case),
                "w" | "W" => Some(FindToggle::Word),
                "r" | "R" => Some(FindToggle::Regex),
                _ => None,
            };
            if let Some(toggle) = toggle {
                self.toggle_find_option(toggle, cx);
                cx.stop_propagation();
                return;
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
            _ => {}
        }
    }

    fn on_find_action(
        &mut self,
        action: FindBarAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            FindBarAction::ToggleCase => self.toggle_find_option(FindToggle::Case, cx),
            FindBarAction::ToggleWord => self.toggle_find_option(FindToggle::Word, cx),
            FindBarAction::ToggleRegex => self.toggle_find_option(FindToggle::Regex, cx),
            FindBarAction::Previous => self.find_previous(cx),
            FindBarAction::Next => self.find_next(cx),
            FindBarAction::Close => self.close_find(window, cx),
        }
    }
}
