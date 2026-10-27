//! Caret, hit-test and selection geometry over a laid-out document.

use std::ops::Range;

use gpui::{Pixels, WrappedLine, point, px};

use super::*;

impl ProseLayout {
    pub fn block_at_y(&self, y: Pixels) -> usize {
        let after = self.blocks.partition_point(|b| b.y + b.height <= y);
        after.min(self.blocks.len().saturating_sub(1))
    }

    /// Caret rectangle (content space) for buffer offset `pos`.
    pub fn caret(&self, pos: usize) -> Option<CaretPlace> {
        let (bi, at) = self.proj.locate(pos);
        let bl = self.blocks.get(bi)?;
        let h = bl.line_height;
        match at {
            Where::Lead(i, byte) => {
                let lead = bl.leads.get(i)?;
                let x = match &lead.line {
                    Some(line) => lead.x + line.x_for_index(byte),
                    None => lead.x,
                };
                Some(CaretPlace {
                    x,
                    y: bl.y + bl.text_top,
                    h,
                })
            }
            Where::Body(d) => {
                if bl.chip.is_some() {
                    let x = if pos > self.proj.block(bi).src.start {
                        bl.x + bl.chip.as_ref().map_or(px(0.), |c| c.width) + px(14.)
                    } else {
                        bl.x - px(4.)
                    };
                    return Some(CaretPlace { x, y: bl.y, h });
                }
                let li = line_for(&bl.line_starts, &bl.lines, d);
                let line = bl.lines.get(li)?;
                let local = d.saturating_sub(bl.line_starts[li]).min(line.len());
                let p = line
                    .position_for_index(local, h)
                    .unwrap_or_else(|| point(px(0.), px(0.)));
                Some(CaretPlace {
                    x: bl.x + p.x,
                    y: bl.y + bl.line_tops[li] + p.y,
                    h,
                })
            }
        }
    }

    /// What is at content-space `(x, y)`.
    pub fn hit(&self, x: Pixels, y: Pixels) -> Hit {
        let bi = self.block_at_y(y);
        let Some(bl) = self.blocks.get(bi) else {
            return Hit::Pos(0);
        };
        let block = self.proj.block(bi);
        let display = &self.proj.displays[bi];
        if bl.chip.is_some() {
            if block.src.end > block.src.start || block.chip.as_ref().is_some_and(|c| !c.folded) {
                return Hit::Chip;
            }
            return Hit::Pos(block.src.start);
        }
        let local_y = y - bl.y;
        if let Some(img) = &bl.image
            && local_y >= img.y
        {
            return Hit::Pos(block.src.end);
        }
        // Leads: only on the first row, left of the body.
        if x < bl.x && local_y < bl.text_top + bl.line_height {
            for (i, lead) in bl.leads.iter().enumerate() {
                if lead.w > px(0.) && x >= lead.x && x < lead.x + lead.w {
                    if let LeadKind::Check(_) = lead.kind {
                        return Hit::Check(display.leads[i].src.clone());
                    }
                    if let Some(line) = &lead.line {
                        let byte = line.closest_index_for_x(x - lead.x);
                        return Hit::Pos(display.src_at(block, Where::Lead(i, byte)));
                    }
                }
            }
        }
        if bl.lines.is_empty() || display.map.is_empty() {
            return Hit::Pos(
                block
                    .src
                    .start
                    .max(display.leads.last().map_or(0, |l| l.src.end)),
            );
        }
        // Find the wrapped source line under y.
        let mut li = 0;
        for (i, top) in bl.line_tops.iter().enumerate() {
            if local_y >= *top {
                li = i;
            }
        }
        let line = &bl.lines[li];
        let rel = point(x - bl.x, (local_y - bl.line_tops[li]).max(px(0.)));
        let idx = match line.closest_index_for_position(rel, bl.line_height) {
            Ok(i) | Err(i) => i,
        };
        let d = bl.line_starts[li] + idx.min(line.len());
        Hit::Pos(display.src_at(block, Where::Body(d)))
    }

    /// Content-space rects covering buffer range `range`.
    pub fn range_rects(&self, range: Range<usize>) -> Vec<(Pixels, Pixels, Pixels, Pixels)> {
        let mut out = Vec::new();
        if range.start >= range.end {
            return out;
        }
        let first = self.proj.doc.block_at(range.start);
        for bi in first..self.blocks.len() {
            let block = self.proj.block(bi);
            if block.src.start > range.end {
                break;
            }
            let bl = &self.blocks[bi];
            let display = &self.proj.displays[bi];
            if bl.chip.is_some() {
                continue;
            }
            let s = range.start.max(block.src.start);
            let e = range.end.min(block.src.end);
            if s >= e && !(range.start <= block.src.start && range.end > block.src.end) {
                continue;
            }
            // Leads covered.
            for (i, lead) in display.leads.iter().enumerate() {
                if lead.src.start < range.end
                    && lead.src.end > range.start
                    && bl.leads[i].w > px(0.)
                {
                    out.push((
                        bl.leads[i].x,
                        bl.y + bl.text_top,
                        bl.leads[i].w,
                        bl.line_height,
                    ));
                }
            }
            let (Where::Body(d0), Where::Body(d1)) = (
                display.locate(block, s.max(block.src.start)),
                display.locate(block, e.min(block.src.end)),
            ) else {
                continue;
            };
            if d1 <= d0 && range.end <= block.src.end {
                continue;
            }
            for (li, line) in bl.lines.iter().enumerate() {
                let ls = bl.line_starts[li];
                let le = ls + line.len();
                let a = d0.max(ls);
                let b = d1.min(le);
                let rows = row_bounds(line);
                let covers_break = d1 > le && d0 <= le;
                for (ri, (rs, re)) in rows.iter().enumerate() {
                    let (ra, rb) = (ls + rs, ls + re);
                    let xa = a.max(ra);
                    let mut xb = b.min(rb);
                    if (d1 <= ra || d0 >= rb.max(ra + 1) && !(d0 == rb && ri + 1 == rows.len()))
                        && !(ri + 1 == rows.len() && covers_break && d0 <= rb)
                    {
                        continue;
                    }
                    let mut x0 = if xa == ra && ri > 0 {
                        px(0.)
                    } else {
                        line.position_for_index(xa - ls, bl.line_height)
                            .map_or(px(0.), |p| p.x)
                    };
                    if xb < xa {
                        xb = xa;
                    }
                    let mut x1 = if xb == ra && ri > 0 {
                        px(0.)
                    } else {
                        line.position_for_index(xb - ls, bl.line_height)
                            .map_or(px(0.), |p| p.x)
                    };
                    if ri + 1 == rows.len() && covers_break {
                        x1 += px(6.);
                    }
                    if x1 < x0 {
                        std::mem::swap(&mut x0, &mut x1);
                    }
                    if x1 - x0 < px(1.) {
                        continue;
                    }
                    out.push((
                        bl.x + x0,
                        bl.y + bl.line_tops[li] + bl.line_height * ri,
                        x1 - x0,
                        bl.line_height,
                    ));
                }
            }
        }
        out
    }
}

/// Visual rows of one wrapped line as `(start, end)` byte offsets.
pub(super) fn row_bounds(line: &WrappedLine) -> Vec<(usize, usize)> {
    let mut starts = vec![0usize];
    for b in line.wrap_boundaries() {
        let run = &line.runs()[b.run_ix];
        starts.push(run.glyphs[b.glyph_ix].index);
    }
    let mut out = Vec::new();
    for (i, s) in starts.iter().enumerate() {
        let e = starts.get(i + 1).copied().unwrap_or(line.len());
        out.push((*s, e));
    }
    out
}

pub(super) fn line_for(starts: &[usize], lines: &[WrappedLine], d: usize) -> usize {
    let mut li = 0;
    for (i, s) in starts.iter().enumerate() {
        if d >= *s && d <= *s + lines[i].len() {
            li = i;
            if d < *s + lines[i].len() || i + 1 == starts.len() {
                return i;
            }
        }
    }
    li
}
