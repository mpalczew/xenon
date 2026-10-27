//! Status strip for markdown files.

use gpui::{
    AnyElement, App, Context, FontStyle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;
use xenon_settings::MarkdownMode;

use super::super::EditorView;
use crate::prose::style;

impl EditorView {
    /// Status strip for markdown files: counts left, Paper/Full, modes right.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub(in crate::view) fn prose_strip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.is_md_prose_capable() || self.prose.chrome_hidden {
            return None;
        }
        self.ensure_prose_prefs();
        let theme = cx.theme();
        let pal = style::palette(
            theme.colors(),
            theme.appearance().is_light(),
            self.prose.paper,
            theme.players().local().cursor,
        );
        let (words, mins) = self.word_stats(cx);
        let mode = self.prose.mode;
        let paper = self.prose.paper;
        let mac = cfg!(target_os = "macos");
        let key = |k: &str| {
            if mac {
                format!("\u{2325}{k}")
            } else {
                format!("Alt+{k}")
            }
        };
        let button = |id: &'static str, label: String, on: bool| {
            let mut b = div()
                .id(id)
                .px_2()
                .py_px()
                .rounded_md()
                .cursor_pointer()
                .text_color(if on { pal.ink } else { pal.dim })
                .child(label);
            if on {
                b = b.bg(pal.tint).font_weight(FontWeight::SEMIBOLD);
            }
            b
        };
        let _ = FontStyle::Normal;
        let this = cx.entity();
        let set = |m: MarkdownMode| {
            let this = this.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                this.update(cx, |v, cx| v.set_markdown_mode(m, cx));
            }
        };
        let paper_btn = {
            let this = this.clone();
            button("prose-paper", format!("Paper {}", key("P")), paper)
                .on_click(move |_, _, cx| this.update(cx, |v, cx| v.toggle_paper(cx)))
        };
        let full_btn = {
            let this = this.clone();
            button("prose-full", format!("Full {}", key("F")), false)
                .on_click(move |_, _, cx| this.update(cx, |v, cx| v.toggle_full_screen(cx)))
        };
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .h(px(28.))
                .px_3()
                .flex_none()
                .bg(pal.bg)
                .border_t_1()
                .border_color(pal.rule)
                .text_size(px(12.))
                .text_color(pal.dim)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(format!("{words} words \u{b7} {mins} min"))
                        .child(paper_btn)
                        .child(full_btn),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(
                            button(
                                "prose-mode-prose",
                                format!("Prose {}", key("1")),
                                mode == MarkdownMode::Prose,
                            )
                            .on_click(set(MarkdownMode::Prose)),
                        )
                        .child(
                            button(
                                "prose-mode-split",
                                format!("Split {}", key("2")),
                                mode == MarkdownMode::Split,
                            )
                            .on_click(set(MarkdownMode::Split)),
                        )
                        .child(
                            button(
                                "prose-mode-source",
                                format!("Source {}", key("3")),
                                mode == MarkdownMode::Source,
                            )
                            .on_click(set(MarkdownMode::Source)),
                        ),
                )
                .into_any_element(),
        )
    }
}
