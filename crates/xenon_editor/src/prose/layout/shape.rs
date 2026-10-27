//! Shaping helpers: text runs, lead items, images, code colors.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    Font, FontStyle, FontWeight, Hsla, Pixels, SharedString, StrikethroughStyle, TextRun,
    UnderlineStyle, WrappedLine, px,
};

use super::*;

pub(super) fn resolve(dir: &Option<PathBuf>, dest: &str) -> PathBuf {
    let p = PathBuf::from(dest);
    if p.is_absolute() {
        p
    } else {
        dir.clone().unwrap_or_default().join(p)
    }
}

pub(super) fn alt_text(display: &Display) -> SharedString {
    let body = display.body.trim();
    let alt = body
        .strip_prefix("![")
        .and_then(|rest| rest.split("](").next())
        .unwrap_or("image");
    SharedString::from(if alt.is_empty() { "image" } else { alt }.to_string())
}

pub(super) fn chip_block(
    bl: &mut BlockLayout,
    block: &Block,
    input: &LayoutInput,
    sys: &Arc<gpui::WindowTextSystem>,
) {
    let chip = block.chip.as_ref();
    let label = match chip {
        Some(c) if c.folded => {
            let mut s = format!("\u{25c7} {} comments", c.comments);
            if c.flags > 0 {
                s.push_str("  \u{2691} flags");
            }
            s
        }
        _ => "\u{25c7} fold comments".to_string(),
    };
    let runs = vec![TextRun {
        len: label.len(),
        font: font_with(&input.body_font, FontWeight::MEDIUM, FontStyle::Normal),
        color: input.palette.dim,
        background_color: None,
        underline: None,
        strikethrough: None,
    }];
    let size = input.size * 0.78;
    bl.chip = Some(sys.shape_line(SharedString::from(label), size, &runs, None));
    bl.size = size;
    bl.height = px(f32::from(input.size) * input.line_height) * 1.15;
    bl.lines = shape_body("", &[], size, None, sys);
    bl.line_starts = vec![0];
    bl.line_tops = vec![px(0.)];
}

pub(super) fn font_with(base: &Font, weight: FontWeight, style: FontStyle) -> Font {
    let mut f = base.clone();
    f.weight = weight;
    f.style = style;
    f
}

pub(super) fn shape_body(
    text: &str,
    runs: &[TextRun],
    size: Pixels,
    wrap: Option<Pixels>,
    sys: &Arc<gpui::WindowTextSystem>,
) -> Vec<WrappedLine> {
    sys.shape_text(SharedString::from(text.to_string()), size, runs, wrap, None)
        .map(|v| v.into_vec())
        .unwrap_or_default()
}

/// A wrap that starts a row on an emphasis or code marker with text before it
/// splits the marker from the word it closes. GPUI breaks before any non-word
/// character, so narrow the column until no row starts that way.
fn splits_marker(text: &str, lines: &[WrappedLine]) -> bool {
    for (line, src) in lines.iter().zip(text.split('\n')) {
        for (start, _) in super::hit::row_bounds(line).into_iter().skip(1) {
            let b = src.as_bytes();
            if matches!(b.get(start), Some(b'*' | b'`')) && start > 0 && b[start - 1] != b' ' {
                return true;
            }
        }
    }
    false
}

pub(super) fn shape_prose(
    text: &str,
    runs: &[TextRun],
    size: Pixels,
    wrap: Pixels,
    sys: &Arc<gpui::WindowTextSystem>,
) -> Vec<WrappedLine> {
    let mut width = wrap;
    let mut lines = shape_body(text, runs, size, Some(width), sys);
    if !text.contains(['*', '`']) {
        return lines;
    }
    for _ in 0..24 {
        if !splits_marker(text, &lines) {
            break;
        }
        width -= size * 0.5;
        lines = shape_body(text, runs, size, Some(width), sys);
    }
    lines
}

pub(super) fn text_runs(
    display: &Display,
    block: &Block,
    input: &LayoutInput,
    _size: Pixels,
    mono_block: bool,
) -> Vec<TextRun> {
    let p = &input.palette;
    let heading = matches!(block.kind, BlockKind::Heading(_));
    let quote = block.quote > 0;
    let colors = code_colors(display, block, input);
    split_runs(&display.runs, &colors)
        .iter()
        .map(|run: &ColoredRun| {
            let f = run.style.flags;
            let base = if mono_block || f & CODE != 0 {
                &input.mono_font
            } else {
                &input.body_font
            };
            let weight = if f & BOLD != 0 || (heading && !run.style.dim) {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            };
            let style = if f & ITALIC != 0 || (quote && !mono_block) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            };
            let mut color: Hsla = run.color.unwrap_or(p.ink);
            if block.done {
                color = p.muted;
            }
            if quote && !mono_block {
                color = p.ink.blend(p.bg.opacity(0.12));
            }
            if f & (LINK | FOOT) != 0 {
                color = p.accent;
            }
            if f & (URL | HTML) != 0 || run.style.dim {
                color = p.dim;
            }
            if f & CODE != 0 && !mono_block {
                color = p.ink;
            }
            let strike = f & STRIKE != 0 || block.done;
            TextRun {
                len: run.len,
                font: font_with(base, weight, style),
                color,
                background_color: (f & CODE != 0 && !mono_block && !run.style.dim)
                    .then_some(p.tint),
                underline: (f & LINK != 0).then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(p.accent.opacity(0.55)),
                    wavy: false,
                }),
                strikethrough: strike.then_some(StrikethroughStyle {
                    thickness: px(1.),
                    color: Some(color),
                }),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lead_layout(
    block: &Block,
    display: &Display,
    input: &LayoutInput,
    sys: &Arc<gpui::WindowTextSystem>,
    size: Pixels,
    _lh: Pixels,
    body_x: Pixels,
    rule_x: Option<Pixels>,
) -> Vec<LeadLayout> {
    let p = &input.palette;
    let mut items: Vec<LeadLayout> = Vec::new();
    for lead in &display.leads {
        let w;
        let mut line = None;
        match lead.kind {
            LeadKind::Check(_) => w = input.size * 1.4,
            _ if lead.text.is_empty() => w = px(0.),
            kind => {
                let heading = matches!(kind, LeadKind::Heading);
                let weight = if heading {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                };
                let mono = matches!(block.kind, BlockKind::Code | BlockKind::Table);
                let runs = vec![TextRun {
                    len: lead.text.len(),
                    font: font_with(
                        if mono {
                            &input.mono_font
                        } else {
                            &input.body_font
                        },
                        weight,
                        FontStyle::Normal,
                    ),
                    color: p.dim,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }];
                let shaped =
                    sys.shape_line(SharedString::from(lead.text.clone()), size, &runs, None);
                w = shaped.width;
                line = Some(shaped);
            }
        }
        items.push(LeadLayout {
            x: px(0.),
            w,
            line,
            kind: lead.kind,
        });
    }
    // Quote marks hang left of the rule; the rest right-align to the body.
    let quote_w: Pixels = items
        .iter()
        .filter(|l| l.kind == LeadKind::Quote)
        .map(|l| l.w)
        .fold(px(0.), |a, b| a + b);
    let item_w: Pixels = items
        .iter()
        .filter(|l| l.kind != LeadKind::Quote)
        .map(|l| l.w)
        .fold(px(0.), |a, b| a + b);
    let mut qx = rule_x.unwrap_or(body_x) - px(6.) - quote_w;
    let mut ix = body_x - item_w;
    for item in &mut items {
        if item.kind == LeadKind::Quote {
            item.x = qx;
            qx += item.w;
        } else {
            item.x = ix;
            ix += item.w;
        }
    }
    items
}

pub(super) struct ColoredRun {
    pub(super) len: usize,
    pub(super) style: RunStyle,
    pub(super) color: Option<Hsla>,
}

/// Syntax colors for a fenced code block, as `(body byte range, color)`.
pub(super) fn code_colors(
    display: &Display,
    block: &Block,
    input: &LayoutInput,
) -> Vec<(Range<usize>, Hsla)> {
    if block.kind != BlockKind::Code {
        return Vec::new();
    }
    let body = &display.body;
    let first_end = body.find('\n').unwrap_or(body.len());
    let first = &body[..first_end];
    let fence = first.trim_start();
    if !(fence.starts_with("```") || fence.starts_with("~~~")) {
        return Vec::new();
    }
    let lang = fence.trim_start_matches(['`', '~']).trim();
    if lang.is_empty() {
        return Vec::new();
    }
    let start = (first_end + 1).min(body.len());
    let last_nl = body
        .rfind('\n')
        .filter(|i| *i >= start)
        .unwrap_or(body.len());
    let end = if body[last_nl.min(body.len())..]
        .trim_start_matches('\n')
        .trim_start()
        .starts_with("```")
        || body[last_nl.min(body.len())..]
            .trim_start_matches('\n')
            .trim_start()
            .starts_with("~~~")
    {
        last_nl
    } else {
        body.len()
    };
    if end <= start {
        return Vec::new();
    }
    (input.highlight)(lang, &body[start..end])
        .into_iter()
        .map(|(r, c)| (r.start + start..r.end + start, c))
        .collect()
}

pub(super) fn split_runs(
    runs: &[crate::prose::display::Run],
    colors: &[(Range<usize>, Hsla)],
) -> Vec<ColoredRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        let mut cursor = at;
        for (range, color) in colors {
            if range.end <= cursor || range.start >= end {
                continue;
            }
            let s = range.start.max(cursor);
            let e = range.end.min(end);
            if s > cursor {
                out.push(ColoredRun {
                    len: s - cursor,
                    style: run.style,
                    color: None,
                });
            }
            out.push(ColoredRun {
                len: e - s,
                style: run.style,
                color: Some(*color),
            });
            cursor = e;
        }
        if cursor < end {
            out.push(ColoredRun {
                len: end - cursor,
                style: run.style,
                color: None,
            });
        }
        at = end;
    }
    out
}
