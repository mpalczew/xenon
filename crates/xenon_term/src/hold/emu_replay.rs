//! `replay()`: plain VT sequences that rebuild the screen in a blank terminal of the
//! same size. Exact for text, SGR, OSC 8 links, soft wraps, the alternate screen, the
//! cursor, modes, palette and title. Not replayed: the scroll region (DECSTBM), tab
//! stops and character sets, which `alacritty_terminal` does not expose.

use std::fmt::Write as _;

use alacritty_terminal::grid::{Cursor, Dimensions};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor};

use super::emu::Emu;
use super::pen::{Pen, clean};

struct AltSnapshot {
    cells: Vec<Cell>,
    cursor: Cursor<Cell>,
    saved: Cursor<Cell>,
}

impl Emu {
    pub(crate) fn replay(&mut self) -> Vec<u8> {
        let mut out = String::new();
        out.push_str("\x1bc"); // RIS
        self.write_palette(&mut out);
        // Origin mode moves the cursor home, so it goes before any positioning. Without
        // the scroll region (see the module note), rows are the same either way.
        if self.mode().contains(TermMode::ORIGIN) {
            out.push_str("\x1b[?6h");
        }
        if self.mode().contains(TermMode::ALT_SCREEN) {
            self.write_both_screens(&mut out);
        } else {
            self.write_screen(&mut out, true);
            self.write_cursor(&mut out);
        }
        self.write_modes(&mut out);
        if let Some(title) = &self.title {
            let _ = write!(out, "\x1b]2;{}\x1b\\", clean(title));
        }
        out.into_bytes()
    }

    /// The alternate screen is active. alacritty keeps the main screen private, so swap
    /// to read it, then swap back and put the alternate screen's cells and cursors back
    /// (entering the alternate screen clears it).
    fn snapshot_alt(&self) -> AltSnapshot {
        let grid = self.term.grid();
        let rows = usize::from(self.size.rows());
        let cols = usize::from(self.size.cols());
        let mut cells = Vec::with_capacity(rows * cols);
        for line in 0..rows {
            for col in 0..cols {
                cells.push(grid[Line(line as i32)][Column(col)].clone());
            }
        }
        AltSnapshot {
            cells,
            cursor: grid.cursor.clone(),
            saved: grid.saved_cursor.clone(),
        }
    }

    fn write_both_screens(&mut self, out: &mut String) {
        let snap = self.snapshot_alt();
        self.term.swap_alt(); // now on the main screen
        self.write_screen(out, true);
        // Park the main cursor where it was: `?1049h` saves it and `?1049l` restores it.
        self.write_cursor(out);
        self.term.swap_alt(); // back on the alternate screen, which is now blank
        out.push_str("\x1b[?1049h");
        self.restore(snap);
        self.write_screen(out, false);
        self.write_cursor(out);
    }

    fn restore(&mut self, snap: AltSnapshot) {
        let cols = usize::from(self.size.cols());
        let grid = self.term.grid_mut();
        for (i, cell) in snap.cells.into_iter().enumerate() {
            grid[Line((i / cols) as i32)][Column(i % cols)] = cell;
        }
        grid.cursor = snap.cursor;
        grid.saved_cursor = snap.saved;
    }

    fn write_palette(&self, out: &mut String) {
        let colors = self.term.colors();
        for index in 0..259 {
            let Some(rgb) = colors[index] else { continue };
            let spec = format!("rgb:{:02x}/{:02x}/{:02x}", rgb.r, rgb.g, rgb.b);
            let _ = match index {
                0..=255 => write!(out, "\x1b]4;{index};{spec}\x1b\\"),
                256 => write!(out, "\x1b]10;{spec}\x1b\\"),
                257 => write!(out, "\x1b]11;{spec}\x1b\\"),
                _ => write!(out, "\x1b]12;{spec}\x1b\\"),
            };
        }
    }

    /// Rows of the active grid. With `history`, the scrollback comes first.
    fn write_screen(&self, out: &mut String, history: bool) {
        let grid = self.term.grid();
        let cols = usize::from(self.size.cols());
        let first = if history {
            -(grid.history_size() as i32)
        } else {
            0
        };
        let last = i32::from(self.size.rows()) - 1;
        let mut pen = Pen::default();
        for line in first..=last {
            let row = &grid[Line(line)];
            let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
            let mut end = cols;
            if !wrapped {
                while end > 0 && is_blank(&row[Column(end - 1)]) {
                    end -= 1;
                }
            }
            for col in 0..end {
                pen.cell(out, &row[Column(col)]);
            }
            if line < last && !wrapped {
                pen.reset(out);
                pen.link(out, None);
                out.push_str("\r\n");
            }
        }
        pen.link(out, None);
        pen.reset(out);
    }

    fn write_cursor(&self, out: &mut String) {
        let grid = self.term.grid();
        let cursor = &grid.cursor;
        let (row, col) = (cursor.point.line.0 + 1, cursor.point.column.0 + 1);
        let _ = write!(out, "\x1b[{row};{col}H");
        // A cursor waiting to wrap sits on the last column. Writing that cell again
        // puts VTE in the same waiting state.
        if cursor.input_needs_wrap && col >= 2 {
            let cell = &grid[Line(cursor.point.line.0)][Column(col - 1)];
            let _ = write!(out, "\x1b[{row};{}H", col - 1);
            let mut pen = Pen::default();
            pen.cell(out, cell);
            pen.link(out, None);
        }
        Pen::default().apply(out, &cursor.template);
    }

    fn write_modes(&self, out: &mut String) {
        let mode = self.mode();
        let dec = |out: &mut String, n: u32, on: bool| {
            let _ = write!(out, "\x1b[?{n}{}", if on { 'h' } else { 'l' });
        };
        let ansi = |out: &mut String, n: u32, on: bool| {
            let _ = write!(out, "\x1b[{n}{}", if on { 'h' } else { 'l' });
        };
        dec(out, 1, mode.contains(TermMode::APP_CURSOR));
        dec(out, 7, mode.contains(TermMode::LINE_WRAP));
        dec(out, 1000, mode.contains(TermMode::MOUSE_REPORT_CLICK));
        dec(out, 1002, mode.contains(TermMode::MOUSE_DRAG));
        dec(out, 1003, mode.contains(TermMode::MOUSE_MOTION));
        dec(out, 1005, mode.contains(TermMode::UTF8_MOUSE));
        dec(out, 1006, mode.contains(TermMode::SGR_MOUSE));
        dec(out, 1007, mode.contains(TermMode::ALTERNATE_SCROLL));
        dec(out, 2004, mode.contains(TermMode::BRACKETED_PASTE));
        out.push_str(if mode.contains(TermMode::APP_KEYPAD) {
            "\x1b="
        } else {
            "\x1b>"
        });
        ansi(out, 20, mode.contains(TermMode::LINE_FEED_NEW_LINE));
        ansi(out, 4, mode.contains(TermMode::INSERT));
        let style = self.term.cursor_style();
        if style != CursorStyle::default() {
            let n = match (style.shape, style.blinking) {
                (CursorShape::Underline, true) => 3,
                (CursorShape::Underline, false) => 4,
                (CursorShape::Beam, true) => 5,
                (CursorShape::Beam, false) => 6,
                (_, true) => 1,
                (_, false) => 2,
            };
            let _ = write!(out, "\x1b[{n} q");
        }
        // Focus reporting is near the end: VTE sends a focus report when ?1004 turns on.
        // The window drops that one report, so reattaching injects no focus event.
        dec(out, 1004, mode.contains(TermMode::FOCUS_IN_OUT));
        dec(out, 25, mode.contains(TermMode::SHOW_CURSOR));
    }
}

fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.bg == Color::Named(NamedColor::Background)
        && cell.extra.is_none()
        && cell.flags.difference(Flags::WRAPLINE).is_empty()
}
