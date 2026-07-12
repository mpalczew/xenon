//! Shared editor chrome bits (context menu rows).

use gpui::{InteractiveElement, ParentElement, Styled, div};

pub(super) fn is_supported_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| gpui::Img::extensions().contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub(super) fn file_title(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Context-menu row: label on the left, keybinding on the right.
pub(super) fn context_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    colors: &theme::ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let hover = colors.element_hover;
    let muted = colors.text_muted;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .px_3()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label)
        .child(div().text_xs().text_color(muted).child(shortcut))
}
