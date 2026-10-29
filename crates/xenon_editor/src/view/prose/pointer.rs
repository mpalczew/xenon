//! Prose mouse, scroll and pane wiring.

use gpui::{
    AnyElement, App, Bounds, Context, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, ScrollWheelEvent, Window, point, px,
};
use theme::ActiveTheme;
use xenon_settings::MarkdownMode;

use super::super::{Content, EditorView};
use super::canvas::prose_canvas;
use crate::mouse;
use crate::prose::edit as prose_edit;
use crate::prose::layout::Hit;
use crate::prose::style;

impl EditorView {
    pub(in crate::view) fn content_point(
        &self,
        position: gpui::Point<Pixels>,
    ) -> gpui::Point<Pixels> {
        point(
            position.x - self.prose.bounds.origin.x,
            position.y - self.prose.bounds.origin.y + self.prose.scroll,
        )
    }

    pub(in crate::view) fn on_prose_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !mouse::is_primary_down(event) {
            return;
        }
        self.claim_keyboard(window, cx);
        if self.vim.mode.is_visual() {
            self.vim.mode = crate::vim::Mode::Normal;
        }
        if self.prose.mode == MarkdownMode::Split {
            self.prose.active_prose = true;
        }
        self.dismiss_menu(cx);
        let Some(layout) = self.prose.layout.clone() else {
            return;
        };
        let p = self.content_point(event.position);
        let hit = layout.hit(p.x, p.y);
        cx.stop_propagation();
        match hit {
            Hit::Check(range) => {
                if let Some(plan) = prose_edit::toggle_at(self.rope(), range.start) {
                    self.end_typing();
                    let keep = match &self.content {
                        Content::Text(b) => b.cursor(),
                        _ => 0,
                    };
                    self.prose_replace(plan.range, &plan.text, cx);
                    if let Content::Text(buffer) = &mut self.content {
                        buffer.set_cursor_raw(keep);
                    }
                }
            }
            Hit::Chip => self.toggle_comments_fold(cx),
            Hit::Pos(pos) => {
                if event.modifiers.secondary() {
                    self.follow_link_at(pos, cx);
                    return;
                }
                let rope = self.rope();
                let line = rope.char_to_line(pos.min(rope.len_chars()));
                let col = pos - rope.line_to_char(line);
                let count = self.click_tracker.count(line, col);
                self.end_typing();
                let extend = event.modifiers.shift;
                let Content::Text(buffer) = &mut self.content else {
                    return;
                };
                match count {
                    2 => {
                        buffer.select_word_at(pos);
                        self.prose.dragging = false;
                    }
                    3 => {
                        buffer.select_line_at(pos);
                        self.prose.dragging = false;
                    }
                    _ => {
                        if extend {
                            buffer.set_anchor_raw(Some(buffer.selection_range().map_or(
                                buffer.cursor(),
                                |r| {
                                    if buffer.cursor() == r.start {
                                        r.end
                                    } else {
                                        r.start
                                    }
                                },
                            )));
                        } else {
                            buffer.set_anchor_raw(None);
                        }
                        buffer.set_cursor_raw(pos);
                        self.prose.dragging = true;
                    }
                }
                self.prose.goal_x = None;
                self.emit_selection(cx);
                self.emit_cursor(cx);
                self.cursor_blink.reset(cx, Self::blink_tick);
                cx.notify();
            }
        }
    }

    pub(in crate::view) fn follow_link_at(&mut self, pos: usize, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let byte = buffer.rope().char_to_byte(pos);
        if let Some(dest) = crate::markdown::link_at(&buffer.text(), byte) {
            self.open_link(&dest, cx);
        }
    }

    pub(in crate::view) fn on_prose_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.prose.dragging || !mouse::is_left_drag(event) {
            return;
        }
        let Some(layout) = self.prose.layout.clone() else {
            return;
        };
        let p = self.content_point(event.position);
        if let Hit::Pos(pos) = layout.hit(p.x, p.y) {
            let Content::Text(buffer) = &mut self.content else {
                return;
            };
            if buffer.selection_range().is_none() && buffer.cursor() != pos {
                buffer.set_anchor_raw(Some(buffer.cursor()));
            }
            buffer.set_cursor_raw(pos);
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(in crate::view) fn on_prose_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.prose.dragging {
            self.prose.dragging = false;
            self.emit_selection(cx);
            cx.notify();
        }
    }

    pub(in crate::view) fn on_prose_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lh = px(xenon_settings::prose(cx).size * xenon_settings::prose(cx).line_height);
        let delta = event.delta.pixel_delta(lh);
        self.prose.scroll = (self.prose.scroll - delta.y).max(px(0.));
        cx.stop_propagation();
        cx.notify();
    }

    // ---- IME helpers (prose-aware bounds) ----

    pub(in crate::view) fn prose_caret_bounds(&self, pos: usize) -> Option<Bounds<Pixels>> {
        let layout = self.prose.layout.as_ref()?;
        let c = layout.caret(pos)?;
        let origin = self.prose.bounds.origin + point(c.x, c.y - self.prose.scroll);
        Some(Bounds::new(origin, gpui::size(px(2.), c.h)))
    }

    // ---- canvas ----

    pub(in crate::view) fn prose_pane(
        &self,
        register_input: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        prose_canvas(cx.entity(), self.focus.clone(), register_input).into_any_element()
    }

    pub(in crate::view) fn page_background(&self, cx: &App) -> gpui::Hsla {
        let theme = cx.theme();
        if self.is_md_prose_capable() && self.prose.paper {
            style::palette(
                theme.colors(),
                theme.appearance().is_light(),
                true,
                theme.players().local().cursor,
            )
            .bg
        } else {
            theme.colors().editor_background
        }
    }
}
