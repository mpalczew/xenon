//! Prose projection, step three: proportional layout. Each block's display
//! text is shaped with GPUI (wrapped to the column); this module also answers
//! "where is the caret for buffer offset N" and "which offset is at (x, y)".
//! Coordinates are content-space: origin at the viewport's top-left, scroll 0.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Font, Hsla, Pixels, RenderImage, ShapedLine, SharedString, Window, WrappedLine, px};

use super::display::{Display, RunStyle, Where};
use super::model::{BOLD, Block, BlockKind, CODE, FOOT, HTML, ITALIC, LINK, LeadKind, STRIKE, URL};
use super::projection::Projection;
use super::style::Palette;

const PAD_X: f32 = 24.;
const CODE_PAD: f32 = 12.;
const CODE_PAD_Y: f32 = 6.;

pub type SyntaxColors = Box<dyn Fn(&str, &str) -> Vec<(Range<usize>, Hsla)>>;

pub struct LayoutInput {
    pub width: Pixels,
    pub size: Pixels,
    pub line_height: f32,
    pub measure: f32,
    pub body_font: Font,
    pub mono_font: Font,
    pub palette: Palette,
    pub doc_dir: Option<PathBuf>,
    /// Syntax colors for a fenced block: `(language, code) -> (byte range, color)`.
    pub highlight: SyntaxColors,
}

pub struct ImageSlot {
    pub alt: SharedString,
    pub image: Option<Arc<RenderImage>>,
    pub w: Pixels,
    pub h: Pixels,
    /// Top of the image area relative to the block.
    pub y: Pixels,
}

pub struct LeadLayout {
    pub x: Pixels,
    pub w: Pixels,
    pub line: Option<ShapedLine>,
    pub kind: LeadKind,
}

pub struct BlockLayout {
    pub y: Pixels,
    pub height: Pixels,
    pub x: Pixels,
    pub w: Pixels,
    pub line_height: Pixels,
    pub size: Pixels,
    /// Top of the text inside the block (tinted blocks pad it).
    pub text_top: Pixels,
    pub lines: Vec<WrappedLine>,
    pub line_starts: Vec<usize>,
    pub line_tops: Vec<Pixels>,
    pub leads: Vec<LeadLayout>,
    /// Tinted box (code, table, comments).
    pub boxed: Option<(Pixels, Pixels)>,
    pub rule_x: Option<Pixels>,
    pub image: Option<ImageSlot>,
    pub chip: Option<ShapedLine>,
    /// Content-space y of hidden table separator lines.
    pub sep_ys: Vec<Pixels>,
}

pub struct ProseLayout {
    pub proj: Projection,
    pub blocks: Vec<BlockLayout>,
    pub height: Pixels,
    pub col_x: Pixels,
    pub col_w: Pixels,
    pub width: Pixels,
}

/// What a click landed on.
pub enum Hit {
    Pos(usize),
    /// Task box: the block's `[ ]` source range.
    Check(Range<usize>),
    Chip,
}

pub struct CaretPlace {
    pub x: Pixels,
    pub y: Pixels,
    pub h: Pixels,
}

fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.7,
        2 => 1.4,
        3 => 1.2,
        _ => 1.08,
    }
}

pub type ImageLoader<'a> = dyn FnMut(&std::path::Path) -> Option<(Arc<RenderImage>, f32, f32)> + 'a;

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub fn layout(
    proj: Projection,
    input: &LayoutInput,
    load_image: &mut ImageLoader,
    window: &mut Window,
) -> ProseLayout {
    let sys = window.text_system().clone();
    let ch = {
        let id = sys.resolve_font(&input.body_font);
        sys.advance(id, input.size, '0')
            .map(|s| s.width)
            .unwrap_or(input.size * 0.55)
    };
    let mono_size = input.size * 0.84;
    let mono_cell = {
        let id = sys.resolve_font(&input.mono_font);
        sys.advance(id, mono_size, 'm')
            .map(|s| s.width)
            .unwrap_or(mono_size * 0.6)
    };
    let avail = (input.width - px(PAD_X * 2.)).max(px(120.));
    let col_w = (ch * input.measure).min(avail);
    let col_x = ((input.width - col_w) / 2.).max(px(PAD_X).min(input.width / 2.));
    let base_lh = px(f32::from(input.size) * input.line_height);
    let list_indent = input.size * 1.7;
    let quote_indent = input.size * 1.25;

    let mut blocks = Vec::with_capacity(proj.doc.blocks.len());
    let mut y = base_lh * 1.6;
    for (block, display) in proj.doc.blocks.iter().zip(&proj.displays) {
        let verbatim = block.kind.verbatim();
        let size = match block.kind {
            BlockKind::Heading(level) => input.size * heading_scale(level),
            _ if verbatim => mono_size,
            _ => input.size,
        };
        let lh = match block.kind {
            BlockKind::Heading(_) => px(f32::from(size) * (input.line_height - 0.2).max(1.25)),
            _ if verbatim => px(f32::from(size) * 1.55),
            _ => base_lh,
        };
        let indent = list_indent * f32::from(block.list) + quote_indent * f32::from(block.quote);
        let body_x = col_x + indent;
        let body_w = (col_w - indent).max(px(80.));
        y += lh * block.kind.space_before();
        let top = y;
        let tinted = matches!(
            block.kind,
            BlockKind::Code | BlockKind::Table | BlockKind::Html
        );
        let mut bl = BlockLayout {
            y: top,
            height: px(0.),
            x: body_x,
            w: body_w,
            line_height: lh,
            size,
            text_top: px(0.),
            lines: Vec::new(),
            line_starts: Vec::new(),
            line_tops: Vec::new(),
            leads: Vec::new(),
            boxed: None,
            rule_x: (block.quote > 0).then(|| body_x - quote_indent * 0.62),
            image: None,
            chip: None,
            sep_ys: Vec::new(),
        };
        match block.kind {
            BlockKind::Blank => {
                bl.height = if block.quote > 0 {
                    base_lh
                } else {
                    base_lh * 0.62
                };
                bl.lines = shape_body("", &[], size, None, &sys);
                bl.line_starts = vec![0];
                bl.line_tops = vec![px(0.)];
            }
            BlockKind::Chip => chip_block(&mut bl, block, input, &sys),
            _ => {
                let text_x;
                let mut wrap = Some(body_w);
                if tinted {
                    let longest = display
                        .body
                        .split('\n')
                        .map(|l| l.chars().count())
                        .max()
                        .unwrap_or(0);
                    let want = mono_cell * (longest as f32) + px(CODE_PAD * 2.);
                    let box_w = (body_w + px(CODE_PAD * 2.))
                        .max(want.min(avail))
                        .min(input.width - px(16.));
                    let spare = box_w - (body_w + px(CODE_PAD * 2.));
                    let left = (body_x - px(CODE_PAD) - spare / 2.).max(px(8.));
                    bl.boxed = Some((left, box_w));
                    text_x = left + px(CODE_PAD);
                    let inner = box_w - px(CODE_PAD * 2.);
                    wrap = if block.kind == BlockKind::Table {
                        None
                    } else {
                        Some(inner)
                    };
                    bl.text_top = px(CODE_PAD_Y);
                    bl.w = inner;
                    bl.x = text_x;
                }
                let runs = text_runs(display, block, input, size, tinted || verbatim);
                let font_size = size;
                bl.lines = shape_body(&display.body, &runs, font_size, wrap, &sys);
                let mut start = 0;
                let mut top_off = bl.text_top;
                for line in &bl.lines {
                    bl.line_starts.push(start);
                    bl.line_tops.push(top_off);
                    start += line.len() + 1;
                    top_off += lh * (line.wrap_boundaries().len() + 1);
                }
                let mut h = top_off + if tinted { px(CODE_PAD_Y) } else { px(0.) };
                for sep in &display.sep_lines {
                    if let Some(top) = bl.line_tops.get(*sep) {
                        bl.sep_ys.push(*top + lh / 2.);
                    }
                }
                bl.leads = lead_layout(block, display, input, &sys, size, lh, bl.x, bl.rule_x);
                if let Some(dest) = &block.image {
                    let path = resolve(&input.doc_dir, dest);
                    let alt = alt_text(display);
                    let (image, w, hh) = match load_image(&path) {
                        Some((img, w, hh)) => {
                            let max_w = f32::from(body_w).max(80.);
                            let scale = (max_w / w).min(420. / hh).min(1.);
                            (Some(img), px(w * scale), px(hh * scale))
                        }
                        None => (None, body_w, px(110.)),
                    };
                    bl.image = Some(ImageSlot {
                        alt,
                        image,
                        w,
                        h: hh,
                        y: h + px(8.),
                    });
                    h += px(8.) + hh + px(4.);
                }
                bl.height = h;
            }
        }
        y += bl.height;
        blocks.push(bl);
    }
    ProseLayout {
        proj,
        blocks,
        height: y + base_lh * 3.,
        col_x,
        col_w,
        width: input.width,
    }
}

mod hit;
mod shape;

use shape::*;
