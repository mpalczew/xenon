//! Prose projection, step four: paint a laid-out document. Pure drawing from
//! the layout; no product decisions live here.

use std::ops::Range;

use gpui::{
    App, BorderStyle, Bounds, ContentMask, Hsla, Pixels, Point, SharedString, Size, TextAlign,
    TextRun, Window, fill, font, point, px, quad, size,
};

use super::layout::ProseLayout;
use super::model::{BlockKind, LeadKind};
use super::style::Palette;

pub struct PaintInput<'a> {
    pub layout: &'a ProseLayout,
    pub palette: Palette,
    pub bounds: Bounds<Pixels>,
    pub scroll: Pixels,
    pub selection: Option<Range<usize>>,
    pub caret: Option<usize>,
    pub show_caret: bool,
    pub hide_markers: bool,
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub fn paint(input: &PaintInput, window: &mut Window, cx: &mut App) {
    let PaintInput {
        layout,
        palette: p,
        bounds,
        scroll,
        ..
    } = input;
    let origin = bounds.origin - point(px(0.), *scroll);
    let top = *scroll;
    let bottom = *scroll + bounds.size.height;
    window.with_content_mask(Some(ContentMask { bounds: *bounds }), |window| {
        if let Some(sel) = &input.selection {
            for (x, y, w, h) in layout.range_rects(sel.clone()) {
                if y + h < top || y > bottom {
                    continue;
                }
                window.paint_quad(fill(
                    Bounds::new(origin + point(x, y), size(w, h)),
                    p.selection,
                ));
            }
        }
        for (bi, bl) in layout.blocks.iter().enumerate() {
            if bl.y + bl.height < top {
                continue;
            }
            if bl.y > bottom {
                break;
            }
            let block = layout.proj.block(bi);
            let display = &layout.proj.displays[bi];
            let at = |x: Pixels, y: Pixels| origin + point(x, y);
            if let Some((left, width)) = bl.boxed {
                window.paint_quad(quad(
                    Bounds::new(at(left, bl.y), size(width, bl.height)),
                    px(6.),
                    p.tint,
                    0.,
                    Hsla::transparent_black(),
                    BorderStyle::Solid,
                ));
            }
            if let Some(rx) = bl.rule_x {
                window.paint_quad(fill(
                    Bounds::new(at(rx, bl.y), size(px(2.), bl.height)),
                    p.rule.blend(p.dim.opacity(0.55)),
                ));
            }
            for y in &bl.sep_ys {
                let (left, width) = bl.boxed.unwrap_or((bl.x, bl.w));
                window.paint_quad(fill(
                    Bounds::new(at(left + px(8.), *y), size(width - px(16.), px(1.))),
                    p.rule,
                ));
            }
            if block.kind == BlockKind::Rule {
                let used = bl
                    .lines
                    .first()
                    .map_or(px(0.), |l| l.unwrapped_layout.width);
                let (x0, x1) = if input.hide_markers {
                    (layout.col_x, layout.col_x + layout.col_w)
                } else {
                    (bl.x + used + px(10.), layout.col_x + layout.col_w)
                };
                if x1 > x0 {
                    window.paint_quad(fill(
                        Bounds::new(at(x0, bl.y + bl.line_height / 2.), size(x1 - x0, px(1.))),
                        p.rule.blend(p.dim.opacity(0.6)),
                    ));
                }
            }
            if let Some(chip) = &bl.chip {
                let pad = px(8.);
                window.paint_quad(quad(
                    Bounds::new(
                        at(bl.x - pad, bl.y + px(2.)),
                        size(chip.width + pad * 2., bl.height - px(4.)),
                    ),
                    px(999.),
                    p.tint,
                    1.,
                    p.rule,
                    BorderStyle::Solid,
                ));
                let _ = chip.paint(
                    at(bl.x, bl.y + (bl.height - bl.line_height * 0.8) / 2.),
                    bl.line_height * 0.8,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            // Leads.
            for (i, lead) in bl.leads.iter().enumerate() {
                let y = bl.y + bl.text_top;
                match lead.kind {
                    LeadKind::Check(done) => {
                        let s = px(15.);
                        let bx = lead.x + (lead.w - s) / 2. - px(3.);
                        let by = y + (bl.line_height - s) / 2.;
                        let r = Bounds::new(at(bx, by), size(s, s));
                        window.paint_quad(quad(
                            r,
                            px(3.5),
                            if done {
                                p.accent
                            } else {
                                Hsla::transparent_black()
                            },
                            1.5,
                            if done { p.accent } else { p.dim },
                            BorderStyle::Solid,
                        ));
                        if done {
                            let text = SharedString::from("\u{2713}");
                            let run = TextRun {
                                len: text.len(),
                                font: font("Helvetica Neue"),
                                color: p.bg,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };
                            let line = window.text_system().shape_line(text, px(12.), &[run], None);
                            let _ = line.paint(
                                at(bx + (s - line.width) / 2., by + (s - px(14.)) / 2.),
                                px(14.),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                    _ => {
                        if let Some(line) = &lead.line {
                            let _ = display.leads.get(i);
                            let _ = line.paint(
                                at(lead.x, y),
                                bl.line_height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                }
            }
            // Body.
            for (li, line) in bl.lines.iter().enumerate() {
                let y = bl.y + bl.line_tops[li];
                if y + bl.line_height * (line.wrap_boundaries().len() + 1) < top || y > bottom {
                    continue;
                }
                let _ = line.paint(
                    at(bl.x, y),
                    bl.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            // Image or placeholder.
            if let Some(img) = &bl.image {
                let area = Bounds::new(at(bl.x, bl.y + img.y), Size::new(img.w, img.h));
                match &img.image {
                    Some(data) => {
                        let _ = window.paint_image(
                            area,
                            gpui::Corners::all(px(4.)),
                            data.clone(),
                            0,
                            false,
                        );
                    }
                    None => {
                        window.paint_quad(quad(
                            area,
                            px(6.),
                            p.tint,
                            1.,
                            p.rule,
                            BorderStyle::Dashed,
                        ));
                        let text = SharedString::from(format!("\u{25a3} {}", img.alt));
                        let run = TextRun {
                            len: text.len(),
                            font: font("Helvetica Neue"),
                            color: p.dim,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line =
                            window
                                .text_system()
                                .shape_line(text, bl.size * 0.8, &[run], None);
                        let _ = line.paint(
                            at(
                                bl.x + (img.w - line.width) / 2.,
                                bl.y + img.y + (img.h - bl.line_height) / 2.,
                            ),
                            bl.line_height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }
            }
        }
        if let (Some(pos), true) = (input.caret, input.show_caret)
            && let Some(c) = layout.caret(pos)
        {
            let r: Point<Pixels> = origin + point(c.x, c.y);
            window.paint_quad(fill(Bounds::new(r, size(px(2.), c.h)), p.caret));
        }
    });
}
