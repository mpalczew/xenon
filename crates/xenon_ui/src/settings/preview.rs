//! Live preview: a miniature Xenon (rail · editor · terminal) drawn with the
//! active theme, faces, sizes, line numbers, and code wrap.

use gpui::{
    App, BoxShadow, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement, Styled,
    StyledText, div, point, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

use super::appearance::active_syntax;

/// The preview draws at 11/14 of each real size so relative changes show.
const SCALE: f32 = 11. / 14.;
const HEIGHT: f32 = 168.;

type Span = (&'static str, Option<&'static str>);

const CODE: [&[Span]; 5] = [
    &[
        ("fn", Some("keyword")),
        (" ", None),
        ("main", Some("function")),
        ("() {", None),
    ],
    &[(
        "    // Drive many agents across workspaces, any terminal harness",
        Some("comment"),
    )],
    &[
        ("    ", None),
        ("let", Some("keyword")),
        (" app = ", None),
        ("Xenon", Some("type")),
        ("::new();", None),
    ],
    &[
        ("    app.", None),
        ("run", Some("function")),
        ("(", None),
        ("\"agents\"", Some("string")),
        (");", None),
    ],
    &[("}", None)],
];

pub(super) fn live_preview(cx: &App) -> impl IntoElement + use<> {
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    div()
        .relative()
        .flex_none()
        .h(px(HEIGHT))
        .flex()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .overflow_hidden()
        .shadow(vec![
            BoxShadow {
                color: accent.opacity(0.14),
                offset: point(px(0.), px(14.)),
                blur_radius: px(40.),
                spread_radius: px(0.),
                inset: false,
            },
            BoxShadow {
                color: accent.opacity(0.10),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            },
        ])
        .child(rail(cx))
        .child(editor(cx))
        .child(terminal(cx))
        .child(live_badge(cx))
}

fn rail(cx: &App) -> impl IntoElement + use<> {
    let colors = cx.theme().colors();
    let ui = xenon_settings::ui_font(cx);
    let size = px(ui.size * SCALE);
    let row = |name: &'static str, active: bool| {
        let paint = xenon_design_system::list_selection(colors, active);
        div()
            .px(px(6.))
            .py(px(3.))
            .rounded_sm()
            .border_l_2()
            .border_color(paint.accent)
            .bg(paint.background)
            .text_color(paint.foreground)
            .font_family(ui.family.clone())
            .text_size(size)
            .child(name)
    };
    div()
        .w(px(140.))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(8.))
        .bg(colors.panel_background)
        .border_r_1()
        .border_color(colors.border)
        .child(row("xenon", true))
        .child(row("scratch", false))
        .child(row("homelab", false))
}

fn editor(cx: &App) -> impl IntoElement + use<> {
    let colors = cx.theme().colors();
    let face = xenon_settings::editor_font(cx);
    let size = face.size * SCALE;
    let numbers = xenon_settings::show_line_numbers(cx);
    let wrap = xenon_settings::wrap_code(cx);
    let mut pane = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(10.))
        .py(px(8.))
        .bg(colors.editor_background)
        .border_r_1()
        .border_color(colors.border)
        .font_family(face.family)
        .text_size(px(size))
        .line_height(px(size * 1.55))
        .text_color(colors.editor_foreground);
    for (index, spans) in CODE.iter().enumerate() {
        let code = div().flex_1().min_w_0().child(highlighted(spans, cx));
        let code = if wrap {
            code
        } else {
            code.whitespace_nowrap().overflow_hidden()
        };
        pane = pane.child(
            div()
                .flex()
                .gap(px(8.))
                .children(numbers.then(|| {
                    div()
                        .flex_none()
                        .text_color(colors.editor_line_number)
                        .child((index + 1).to_string())
                }))
                .child(code),
        );
    }
    pane
}

fn highlighted(spans: &[Span], cx: &App) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for (piece, scope) in spans {
        let start = text.len();
        text.push_str(piece);
        if let Some(scope) = scope {
            highlights.push((start..text.len(), style_for(scope, cx)));
        }
    }
    StyledText::new(text).with_highlights(highlights)
}

fn style_for(scope: &str, cx: &App) -> HighlightStyle {
    HighlightStyle {
        color: Some(active_syntax(cx, scope)),
        font_style: (scope == "comment").then_some(gpui::FontStyle::Italic),
        ..Default::default()
    }
}

fn terminal(cx: &App) -> impl IntoElement + use<> {
    let colors = cx.theme().colors();
    let face = xenon_settings::terminal_font(cx);
    let size = face.size * SCALE;
    let line = |text: &'static str, color: Hsla| div().text_color(color).child(text);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(10.))
        .py(px(8.))
        .bg(colors.terminal_background)
        .font_family(face.family)
        .text_size(px(size))
        .line_height(px(size * 1.55))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(line("❯ cargo test", colors.terminal_foreground))
        .child(line("   Compiling xenon_ui", colors.terminal_foreground))
        .child(line(
            "test result: ok. 212 passed",
            colors.terminal_ansi_green,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .text_color(colors.terminal_foreground)
                .child("❯ ")
                .child(
                    div()
                        .w(px(size * 0.6))
                        .h(px(size * 1.2))
                        .bg(colors.terminal_foreground),
                ),
        )
}

fn live_badge(cx: &App) -> impl IntoElement + use<> {
    let accent = cx.theme().colors().text_accent;
    div()
        .absolute()
        .right(px(8.))
        .bottom(px(8.))
        .px(px(6.))
        .rounded_sm()
        .bg(accent.opacity(0.16))
        .type_role(TypeRole::ControlLabel, cx)
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD)
        .text_color(accent)
        .child("LIVE")
}
