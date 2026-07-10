//! Mouse hit-testing and multi-click tracking for the text editor.

use std::time::{Duration, Instant};

use gpui::{MouseButton, MouseDownEvent, Pixels, Point, px};

const MULTI_CLICK: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
pub struct ClickLayout {
    pub text_origin: Point<Pixels>,
    pub line_height: Pixels,
    pub scroll_top: Pixels,
    pub cell_width: Pixels,
}

/// Map a window position to a zero-based (row, col) in the buffer.
pub fn position_at(layout: ClickLayout, position: Point<Pixels>) -> (usize, usize) {
    let row = ((position.y - layout.text_origin.y + layout.scroll_top) / layout.line_height)
        .floor()
        .max(0.) as usize;
    let col = ((position.x - layout.text_origin.x) / layout.cell_width + 0.5)
        .floor()
        .max(0.) as usize;
    (row, col)
}

#[derive(Default)]
pub struct ClickTracker {
    last: Option<ClickStamp>,
}

struct ClickStamp {
    at: Instant,
    row: usize,
    col: usize,
    count: u8,
}

impl ClickTracker {
    /// Returns the click count (1, 2, or 3) for this down event.
    pub fn count(&mut self, row: usize, col: usize) -> u8 {
        let now = Instant::now();
        let count = match &self.last {
            Some(prev)
                if now.duration_since(prev.at) <= MULTI_CLICK
                    && prev.row == row
                    && prev.col == col =>
            {
                (prev.count % 3) + 1
            }
            _ => 1,
        };
        self.last = Some(ClickStamp {
            at: now,
            row,
            col,
            count,
        });
        count
    }
}

pub fn is_left_drag(event: &gpui::MouseMoveEvent) -> bool {
    event.pressed_button == Some(MouseButton::Left)
}

/// Pixel distance used only for API symmetry with drag start; currently unused.
#[allow(dead_code)]
pub fn near(a: Point<Pixels>, b: Point<Pixels>) -> bool {
    (a.x - b.x).abs() < px(4.) && (a.y - b.y).abs() < px(4.)
}

pub fn is_primary_down(event: &MouseDownEvent) -> bool {
    event.button == MouseButton::Left
}
