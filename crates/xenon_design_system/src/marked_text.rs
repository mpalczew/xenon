//! One line of prose with inline emphasis: bold, italic, code, strike, link.

use std::ops::Range;

use gpui::{
    App, FontStyle, FontWeight, HighlightStyle, SharedString, StrikethroughStyle, StyledText, px,
};
use theme::ActiveTheme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
}

/// Display text plus byte ranges of emphasis within it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkedText {
    pub text: SharedString,
    pub marks: Vec<(Range<usize>, Marks)>,
}

impl From<&str> for MarkedText {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_owned().into(),
            marks: Vec::new(),
        }
    }
}

pub fn marked_text(marked: MarkedText, cx: &App) -> StyledText {
    let colors = cx.theme().colors();
    let (accent, code_bg) = (colors.text_accent, colors.surface_background);
    let highlights = marked.marks.into_iter().map(move |(range, marks)| {
        let style = HighlightStyle {
            font_weight: marks.bold.then_some(FontWeight::BOLD),
            font_style: marks.italic.then_some(FontStyle::Italic),
            color: (marks.code || marks.link).then_some(accent),
            background_color: marks.code.then_some(code_bg),
            strikethrough: marks.strike.then_some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }),
            ..Default::default()
        };
        (range, style)
    });
    StyledText::new(marked.text).with_highlights(highlights)
}
