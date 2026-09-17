//! In-app menu rows. The only constructor requires a keyboard shortcut.

use gpui::{
    ElementId, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div,
    transparent_black,
};
use theme::ThemeColors;

/// Display form of a key chord (`⌘⇧O`). Empty is illegal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    keys: &'static str,
}

impl Shortcut {
    pub const fn new(keys: &'static str) -> Self {
        assert!(!keys.is_empty(), "menu rows must show a keyboard shortcut");
        Self { keys }
    }

    pub const fn keys(self) -> &'static str {
        self.keys
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
        assert_eq!(Shortcut::new("⌘⇧O").keys(), "⌘⇧O");
    }

    #[test]
    #[should_panic(expected = "menu rows must show a keyboard shortcut")]
    fn shortcut_rejects_empty() {
        let _ = Shortcut::new("");
    }
}
