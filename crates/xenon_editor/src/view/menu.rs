//! Shared editor chrome bits (context menu rows, vim mode bar).

use gpui::{
    App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    StatefulInteractiveElement, Styled, anchored, deferred, div, px,
};

use super::EditorView;

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
    // Prefer element_selected: element_hover often matches elevated_surface.
    let hover = colors.element_selected;
    let muted = colors.text_muted;
    let text = colors.text;
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
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label)
        .child(div().text_xs().text_color(muted).child(shortcut))
}

impl EditorView {
    pub(super) fn vim_mode_bar(
        &self,
        colors: &theme::ThemeColors,
        cx: &App,
    ) -> Option<impl IntoElement + use<>> {
        xenon_settings::vim_mode(cx).then(|| {
            let label = if let Some(draft) = &self.vim.ex_draft {
                format!(":{}", draft.line)
            } else if let Some(draft) = &self.vim.search_draft {
                let prefix = if draft.forward { '/' } else { '?' };
                if draft.pattern.is_empty() {
                    format!("{prefix}")
                } else if draft.has_match {
                    format!("{prefix}{}", draft.pattern)
                } else {
                    format!("{prefix}{}  [no match]", draft.pattern)
                }
            } else if let Some(status) = &self.vim.ex_status {
                format!("{}  |  {status}", self.vim.mode.label())
            } else {
                self.vim.mode.label().to_string()
            };
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(colors.text_muted)
                .border_t_1()
                .border_color(colors.border)
                .child(label)
        })
    }

    pub(super) fn render_context_menu(
        &self,
        position: Point<Pixels>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .child(
                context_item("editor-menu-cut", "Cut", "⌘X", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cut_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("editor-menu-copy", "Copy", "⌘C", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copy_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("editor-menu-paste", "Paste", "⌘V", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.paste_clipboard(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("editor-menu-select-all", "Select All", "⌘A", colors).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.select_all(cx);
                        this.dismiss_menu(cx);
                    }),
                ),
            );
        self.context_menu_shell(position, menu_box, cx)
    }

    /// Preview is read-only: Copy + Select All only.
    pub(super) fn render_preview_context_menu(
        &self,
        position: Point<Pixels>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .child(
                context_item("preview-menu-copy", "Copy", "⌘C", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copy_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("preview-menu-select-all", "Select All", "⌘A", colors).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.select_all(cx);
                        this.dismiss_menu(cx);
                    }),
                ),
            );
        self.context_menu_shell(position, menu_box, cx)
    }

    fn context_menu_shell(
        &self,
        position: Point<Pixels>,
        menu_box: gpui::Div,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .child(deferred(anchored().position(position).child(menu_box)).with_priority(1))
    }
}
