//! In-app menu rows. The only constructor requires a keyboard shortcut.

use gpui::{
    ElementId, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div,
    transparent_black,
};
use theme::ThemeColors;

/// A key chord written as the macOS glyphs (`⌘⇧O`). Empty is illegal.
/// [`Shortcut::keys`] gives what the current platform and keymap show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    keys: &'static str,
}

impl Shortcut {
    pub const fn new(keys: &'static str) -> Self {
        assert!(!keys.is_empty(), "menu rows must show a keyboard shortcut");
        Self { keys }
    }

    pub fn keys(self) -> String {
        xenon_keymap::display_keys(self.keys)
    }
}

/// Label on the left, shortcut on the right. Callers attach `.on_click`.
pub fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    shortcut: Shortcut,
    colors: &ThemeColors,
    selected: bool,
) -> Stateful<gpui::Div> {
    let hover = colors.element_selected;
    let muted = colors.text_muted;
    let text = colors.text;
    let bg = if selected {
        colors.element_selected
    } else {
        transparent_black()
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .px_3()
        .py_1()
        .text_sm()
        .text_color(text)
        .bg(bg)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label.into())
        .child(div().text_xs().text_color(muted).child(shortcut.keys()))
}

#[cfg(test)]
mod tests {
    use super::Shortcut;

    #[test]
    fn shortcut_keeps_keys() {
        assert_eq!(
            Shortcut::new("⌘⇧O").keys(),
            xenon_keymap::display_keys("⌘⇧O")
        );
    }

    #[test]
    #[should_panic(expected = "menu rows must show a keyboard shortcut")]
    fn shortcut_rejects_empty() {
        let _ = Shortcut::new("");
    }
}
