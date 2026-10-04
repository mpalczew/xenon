//! A phone tap as a left click, so a TUI with mouse mode can take it.
//! No mouse mode: the tap does nothing here, and does not start a Mac selection.

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, point, px};

use super::*;

impl TerminalView {
    /// `col` and `row` are cells in the live viewport, origin at the top left.
    pub fn click_from_phone(
        &mut self,
        col: u16,
        row: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        let sent = terminal.update(cx, |terminal, cx| {
            terminal.sync(window, cx);
            if !terminal.mouse_mode(false) {
                return false;
            }
            let (origin, cell_w, line_h, cols, rows) = {
                let bounds = &terminal.last_content().terminal_bounds;
                let cell_w = bounds.cell_width();
                let line_h = bounds.line_height();
                if cell_w == px(0.) || line_h == px(0.) {
                    return false;
                }
                // num_columns rounds the same way the grid is built. A plain
                // cast drops the last cell when n * cell / cell is n - epsilon.
                let cols = bounds.num_columns() as u16;
                let rows = bounds.num_lines() as u16;
                (bounds.bounds.origin, cell_w, line_h, cols, rows)
            };
            if col >= cols || row >= rows {
                return false;
            }
            let position = origin
                + point(
                    cell_w * col as f32 + cell_w * 0.5,
                    line_h * row as f32 + line_h * 0.5,
                );
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            };
            let up = MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            };
            terminal.mouse_down(&down, cx);
            terminal.mouse_up(&up, cx);
            true
        });
        if sent {
            self.note_interaction(cx);
            cx.notify();
        }
    }
}
