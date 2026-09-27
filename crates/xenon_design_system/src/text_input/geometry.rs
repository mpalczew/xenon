use gpui::{Bounds, Pixels, Point, TextRun, Window, WrappedLine, point, px, size};

use super::{TextInputAppearance, TextInputView};

/// Matches the padding on the field chrome. `px_3` is 0.75rem and `py_2` / `p_2`
/// are 0.5rem, and the window rem is the UI font size, not 16px.
pub(super) fn field_insets(appearance: TextInputAppearance, rem: Pixels) -> (Pixels, Pixels) {
    match appearance {
        TextInputAppearance::Bordered => (rem * 0.5, rem * 0.5),
        TextInputAppearance::Palette => (rem * 0.75, rem * 0.5),
        TextInputAppearance::Inline => (px(0.), px(0.)),
    }
}

struct TextLine {
    text: String,
    first_char: usize,
    top: Pixels,
    shape: WrappedLine,
}

pub(super) struct TextGeometry {
    bounds: Bounds<Pixels>,
    origin: Point<Pixels>,
    line_height: Pixels,
    lines: Vec<TextLine>,
}

impl TextInputView {
    pub(super) fn geometry_for_bounds(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> TextGeometry {
        let (horizontal, vertical) = field_insets(self.config.appearance, window.rem_size());
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let text = self.value.text();
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let width = (bounds.size.width - horizontal * 2.).max(px(1.));
        let shaped = window
            .text_system()
            .shape_text(text.to_owned().into(), font_size, &[run], Some(width), None)
            .unwrap_or_default();
        let mut first_char = 0;
        let mut top = px(0.);
        let mut lines: Vec<_> = shaped
            .into_iter()
            .map(|shape| {
                let line = TextLine {
                    text: shape.text.to_string(),
                    first_char,
                    top,
                    shape,
                };
                first_char += line.text.chars().count() + 1;
                top += line.shape.size(line_height).height;
                line
            })
            .collect();
        if lines.is_empty() {
            lines.push(TextLine {
                text: String::new(),
                first_char: 0,
                top: px(0.),
                shape: WrappedLine::default(),
            });
        }
        TextGeometry {
            bounds,
            origin: point(bounds.left() + horizontal, bounds.top() + vertical),
            line_height,
            lines,
        }
    }
}

impl TextGeometry {
    pub(super) fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    pub(super) fn caret_offset(&self, text: &str, utf16: usize) -> Point<Pixels> {
        let rect = self.bounds_for_utf16_range(text, utf16..utf16);
        point(
            rect.left() - self.bounds.left(),
            rect.top() - self.bounds.top(),
        )
    }
    pub(super) fn index_for_point(&self, position: Point<Pixels>) -> usize {
        let x = position.x - self.origin.x;
        let y = position.y - self.origin.y;
        let line = self
            .lines
            .iter()
            .rfind(|line| line.top <= y)
            .unwrap_or(&self.lines[0]);
        let local = point(x, (y - line.top).max(px(0.)));
        let byte = line
            .shape
            .closest_index_for_position(local, self.line_height)
            .unwrap_or_else(|index| index)
            .min(line.text.len());
        line.first_char + line.text[..byte].chars().count()
    }

    pub(super) fn bounds_for_utf16_range(
        &self,
        text: &str,
        range: std::ops::Range<usize>,
    ) -> Bounds<Pixels> {
        let char_index = |offset: usize| {
            let mut units = 0;
            for (i, ch) in text.chars().enumerate() {
                if units >= offset {
                    return i;
                }
                units += ch.len_utf16();
            }
            text.chars().count()
        };
        let locate = |index: usize| {
            let line = self
                .lines
                .iter()
                .rfind(|line| line.first_char <= index)
                .unwrap_or(&self.lines[0]);
            let local = index
                .saturating_sub(line.first_char)
                .min(line.text.chars().count());
            let byte = line
                .text
                .char_indices()
                .nth(local)
                .map_or(line.text.len(), |(byte, _)| byte);
            let position = line
                .shape
                .position_for_index(byte, self.line_height)
                .unwrap_or(point(px(0.), px(0.)));
            point(position.x, line.top + position.y)
        };
        let start = locate(char_index(range.start));
        let end = locate(char_index(range.end));
        let width = if start.y == end.y {
            (end.x - start.x).max(px(1.))
        } else {
            px(1.)
        };
        Bounds::new(
            point(self.origin.x + start.x, self.origin.y + start.y),
            size(width, self.line_height),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_insets_follow_the_ui_rem() {
        let (x, y) = field_insets(TextInputAppearance::Palette, px(14.));
        assert_eq!(x, px(10.5));
        assert_eq!(y, px(7.));
    }
}
