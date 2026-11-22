use crate::ProseFont;

pub const PROSE_LINE_HEIGHTS: [f32; 4] = [1.4, 1.55, 1.7, 1.9];
pub const PROSE_FONTS: [ProseFont; 4] = [
    ProseFont::Auto,
    ProseFont::Serif,
    ProseFont::Sans,
    ProseFont::Mono,
];

pub fn nearest_index(values: &[f32], current: f32) -> usize {
    values
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - current).abs().total_cmp(&(b.1 - current).abs()))
        .map_or(0, |(i, _)| i)
}

pub fn format_size(size: f32) -> String {
    if (size - size.round()).abs() < 0.01 {
        format!("{}", size.round() as i32)
    } else {
        format!("{size:.1}")
    }
}
