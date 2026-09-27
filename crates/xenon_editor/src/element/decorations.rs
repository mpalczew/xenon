use gpui::{Bounds, Hsla, Pixels, point, px, size};

use super::{HitLayout, LayoutInput};

pub struct DiagnosticRange {
    pub range: std::ops::Range<usize>,
    pub color: Hsla,
}

pub type PaintRect = (Bounds<Pixels>, Hsla);

pub(super) struct Decorations {
    pub occurrences: Vec<Bounds<Pixels>>,
    pub underlines: Vec<PaintRect>,
    pub marks: Vec<PaintRect>,
}

pub(super) fn layout_decorations(
    hits: &HitLayout<'_>,
    input: &LayoutInput<'_>,
    gutter: Option<&Bounds<Pixels>>,
) -> Decorations {
    let occurrences = input
        .occurrences
        .iter()
        .flat_map(|range| hits.rects(Some(range)))
        .collect();
    let mut underlines = Vec::new();
    let mut marks = Vec::new();
    for diagnostic in input.diagnostics {
        let row = hits
            .rope
            .char_to_line(diagnostic.range.start.min(hits.rope.len_chars()));
        for rect in hits.rects(Some(&diagnostic.range)) {
            let y = rect.origin.y + rect.size.height - px(2.);
            underlines.push((
                Bounds::new(point(rect.origin.x, y), size(rect.size.width, px(2.))),
                diagnostic.color,
            ));
        }
        let visual = hits
            .wrap_rows
            .and_then(|rows| rows.iter().position(|wrap| wrap.line == row))
            .unwrap_or(row);
        if let Some(gutter) = gutter
            && visual >= hits.first_row
            && visual < hits.last_row
        {
            let y = hits.origin_y + hits.line_height * (visual as f32) - hits.scroll_top;
            marks.push((
                Bounds::new(
                    point(gutter.origin.x + px(2.), y + px(2.)),
                    size(px(3.), hits.line_height - px(4.)),
                ),
                diagnostic.color,
            ));
        }
    }
    Decorations {
        occurrences,
        underlines,
        marks,
    }
}
