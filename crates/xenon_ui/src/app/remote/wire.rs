//! Terminal snapshot → wire screen (pure).

use xenon_remote::{Cell, CursorShapeWire, CursorWire, Rgb, Screen, Style, style_flags};
use xenon_terminal::{ScreenCursorShape, ScreenSnapshot};

/// Terminal snapshot → wire screen.
pub(super) fn wire_screen(snapshot: &ScreenSnapshot) -> Screen {
    Screen {
        cols: snapshot.cols,
        rows: snapshot
            .rows
            .iter()
            .map(|row| row.iter().map(wire_cell).collect())
            .collect(),
        cursor: snapshot.cursor.map(|c| CursorWire {
            r: c.row,
            c: c.col,
            s: match c.shape {
                ScreenCursorShape::Block => CursorShapeWire::Block,
                ScreenCursorShape::Bar => CursorShapeWire::Bar,
                ScreenCursorShape::Underline => CursorShapeWire::Underline,
            },
        }),
    }
}

fn wire_cell(cell: &xenon_terminal::ScreenCell) -> Cell {
    let rgb = |c: [u8; 3]| Rgb(c[0], c[1], c[2]);
    let a = cell.attrs;
    let flags = [
        (a.bold, style_flags::BOLD),
        (a.italic, style_flags::ITALIC),
        (a.underline, style_flags::UNDERLINE),
        (a.dim, style_flags::DIM),
        (a.strike, style_flags::STRIKE),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(0, |acc, (_, bit)| acc | bit);
    Cell {
        ch: cell.ch,
        style: Style {
            fg: cell.fg.map(rgb),
            bg: cell.bg.map(rgb),
            flags,
        },
    }
}
