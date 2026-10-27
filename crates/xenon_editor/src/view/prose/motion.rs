//! Prose caret motion.

use gpui::Context;

use super::super::{Content, EditorView};
use super::*;

impl EditorView {
    pub(super) fn move_to(
        &mut self,
        pos: usize,
        extend: bool,
        keep_goal: bool,
        cx: &mut Context<Self>,
    ) {
        self.end_typing();
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if extend {
            let anchor = buffer.selection_range().map_or(buffer.cursor(), |r| {
                if buffer.cursor() == r.start {
                    r.end
                } else {
                    r.start
                }
            });
            buffer.set_anchor_raw(Some(anchor));
            buffer.set_cursor_raw(pos);
            if anchor == pos {
                buffer.set_anchor_raw(None);
            }
        } else {
            buffer.set_anchor_raw(None);
            buffer.set_cursor_raw(pos);
        }
        if !keep_goal {
            self.prose.goal_x = None;
        }
        self.emit_selection(cx);
        self.emit_cursor(cx);
        self.cursor_blink.reset(cx, Self::blink_tick);
        cx.notify();
    }

    pub(super) fn vertical(&mut self, dir: i32, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.prose.layout.clone() else {
            return;
        };
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let cursor = buffer.cursor();
        let Some(here) = layout.caret(cursor) else {
            return;
        };
        let goal = self.prose.goal_x.unwrap_or(here.x);
        let mut ty = here.y + here.h / 2.;
        let mut target = None;
        for _ in 0..8 {
            ty += here.h * 0.5 * dir as f32;
            if ty < px(0.) {
                target = Some(0);
                break;
            }
            if ty > layout.height {
                target = Some(buffer.rope().len_chars());
                break;
            }
            if let Hit::Pos(p) = layout.hit(goal, ty)
                && let Some(c) = layout.caret(p)
                && (c.y - here.y).abs() > px(1.)
            {
                target = Some(p);
                break;
            }
        }
        let Some(pos) = target else { return };
        self.prose.goal_x = Some(goal);
        self.move_to(pos, extend, true, cx);
    }

    pub(super) fn row_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.prose.layout.clone() else {
            return;
        };
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let Some(c) = layout.caret(buffer.cursor()) else {
            return;
        };
        let x = if end { layout.width * 2. } else { px(0.) };
        if let Hit::Pos(p) = layout.hit(x, c.y + c.h / 2.) {
            self.move_to(p, extend, false, cx);
        }
    }

    pub(super) fn word_move(&mut self, dir: i32, extend: bool, cx: &mut Context<Self>) {
        self.end_typing();
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let motion = if dir < 0 {
            Motion::WordLeft
        } else {
            Motion::WordRight
        };
        buffer.apply(if extend {
            EditCommand::Extend(motion)
        } else {
            EditCommand::Move(motion)
        });
        let pos = buffer.cursor();
        if let Some(proj) = self.fresh_projection(cx)
            && !proj.valid(pos)
        {
            let snapped = proj.step(pos, 1, self.rope_len());
            if let Content::Text(buffer) = &mut self.content {
                buffer.set_cursor_raw(snapped);
            }
        }
        self.emit_cursor(cx);
        cx.notify();
    }
}
