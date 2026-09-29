//! A three-line code snippet painted in one theme's own colors.

use gpui::{
    App, BoxShadow, InteractiveElement, ParentElement, SharedString, Styled, StyledText, div,
    point, px,
};
use theme::{Appearance, Theme, ThemeRegistry};
use xenon_design_system::{TypeRole, Typography};

const HEIGHT: f32 = 58.;

pub(super) struct TileState {
    pub selected: bool,
}

/// `None` paints an empty slot (a family without that appearance).
pub(super) fn tile(
    name: Option<&'static str>,
    half: Appearance,
    state: TileState,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let id = SharedString::from(format!("theme-tile-{}", name.unwrap_or("none")));
    let Some(theme) = name.and_then(|n| ThemeRegistry::global(cx).get(n).ok()) else {
        return empty(id, cx);
    };
    let colors = theme.colors();
    let ring = if state.selected {
        colors.text_accent
    } else {
        colors.border
    };
    let mut tile = div()
        .id(id)
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(HEIGHT))
        .px(px(8.))
        .py(px(6.))
        .rounded_md()
        .border_1()
        .border_color(ring)
        .bg(colors.editor_background)
        .text_color(colors.editor_foreground)
        .font_family(xenon_settings::editor_font(cx).family)
        .text_size(px(11.))
        .line_height(px(15.))
        .overflow_hidden()
        .cursor_pointer()
        .child(snippet(&theme, 0))
        .child(snippet(&theme, 1))
        .child(snippet(&theme, 2))
        .child(
            div()
                .absolute()
                .right(px(6.))
                .bottom(px(4.))
                .type_role(TypeRole::ControlLabel, cx)
                .text_size(px(10.))
                .text_color(colors.text_muted)
                .child(match half {
                    Appearance::Dark => "Dark",
                    Appearance::Light => "Light",
                }),
        );
    if state.selected {
        tile = tile.shadow(vec![BoxShadow {
            color: colors.text_accent.opacity(0.35),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(2.),
            inset: false,
        }]);
    }
    tile
}

fn snippet(theme: &Theme, line: usize) -> StyledText {
    let spans: &[(&str, Option<&str>)] = match line {
        0 => &[
            ("fn", Some("keyword")),
            (" ", None),
            ("spawn", Some("function")),
            ("() {", None),
        ],
        1 => &[("    ", None), ("\"claude\"", Some("string"))],
        _ => &[("}", None)],
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    for (piece, scope) in spans {
        let start = text.len();
        text.push_str(piece);
        if let Some(scope) = scope
            && let Some(style) = theme.syntax().style_for_name(scope)
        {
            highlights.push((start..text.len(), style));
        }
    }
    StyledText::new(text).with_highlights(highlights)
}

fn empty(id: SharedString, cx: &App) -> gpui::Stateful<gpui::Div> {
    use theme::ActiveTheme;
    let colors = cx.theme().colors();
    div()
        .id(id)
        .flex_1()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_dashed()
        .border_color(colors.border)
        .type_role(TypeRole::ControlLabel, cx)
        .text_color(colors.text_muted)
        .child("Dark only")
}
