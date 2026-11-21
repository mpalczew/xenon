//! The service's own screen: an `alacritty_terminal` emulator plus `replay()`.
//!
//! `replay()` writes plain VT sequences that rebuild this screen in a blank terminal.

use std::fmt::Write as _;
use std::sync::mpsc::{Receiver, Sender, channel};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Cursor, Dimensions};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor, Processor, Rgb};

use crate::Size;
use crate::pen::{Pen, clean};

const SCROLLBACK: usize = 10_000;

struct Listener(Sender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

struct Dim(Size);

impl Dimensions for Dim {
    fn total_lines(&self) -> usize {
        usize::from(self.0.rows)
    }
    fn screen_lines(&self) -> usize {
        usize::from(self.0.rows)
    }
    fn columns(&self) -> usize {
        usize::from(self.0.cols)
    }
}

/// One terminal's screen state.
pub struct Emu {
    term: Term<Listener>,
    parser: Processor,
    events: Receiver<Event>,
    size: Size,
    title: Option<String>,
}

struct AltSnapshot {
    cells: Vec<Cell>,
    cursor: Cursor<Cell>,
    saved: Cursor<Cell>,
}

impl Emu {
    pub fn new(size: Size) -> Self {
        let (tx, events) = channel();
        let config = Config {
            scrolling_history: SCROLLBACK,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &Dim(size), Listener(tx)),
            parser: Processor::new(),
            events,
            size,
            title: None,
        }
    }

    pub fn resize(&mut self, size: Size) {
        if size != self.size {
            self.term.resize(Dim(size));
            self.size = size;
        }
    }

    /// Feed program output. Returns the bytes the emulator wants written back to the
    /// program (replies to queries). The caller drops them when a window answers instead.
    pub fn advance(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        let mut replies = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Title(t) => self.title = Some(t),
                Event::ResetTitle => self.title = None,
                Event::PtyWrite(text) => replies.extend_from_slice(text.as_bytes()),
                Event::ColorRequest(index, format) => {
                    let rgb = self.term.colors()[index].unwrap_or_else(|| default_color(index));
                    replies.extend_from_slice(format(rgb).as_bytes());
                }
                Event::TextAreaSizeRequest(format) => {
                    let size = WindowSize {
                        num_lines: self.size.rows,
                        num_cols: self.size.cols,
                        cell_width: 1,
                        cell_height: 1,
                    };
                    replies.extend_from_slice(format(size).as_bytes());
                }
                // Not implemented in the spike: clipboard store and load (OSC 52), bell.
                _ => {}
            }
        }
        replies
    }

    /// Standard VT that rebuilds this screen in a blank terminal of the same size.
    pub fn replay(&mut self) -> Vec<u8> {
        let mut out = String::new();
        out.push_str("\x1bc"); // RIS
        self.write_palette(&mut out);
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
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
        let rows = usize::from(self.size.rows);
        let cols = usize::from(self.size.cols);
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
        let cols = usize::from(self.size.cols);
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
        let cols = usize::from(self.size.cols);
        let first = if history {
            -(grid.history_size() as i32)
        } else {
            0
        };
        let last = i32::from(self.size.rows) - 1;
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
        let mode = *self.term.mode();
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
        dec(out, 1004, mode.contains(TermMode::FOCUS_IN_OUT));
        dec(out, 25, mode.contains(TermMode::SHOW_CURSOR));
        // Not implemented in the spike: the scroll region (DECSTBM), origin mode, tab
        // stops and character sets. alacritty_terminal does not expose them.
    }
}

fn default_color(index: usize) -> Rgb {
    // Only used when no window is attached to answer and nothing set the colour.
    match index {
        257 => Rgb { r: 0, g: 0, b: 0 },
        _ => Rgb {
            r: 0xdd,
            g: 0xdd,
            b: 0xdd,
        },
    }
}

fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.bg == Color::Named(NamedColor::Background)
        && cell.extra.is_none()
        && cell.flags.difference(Flags::WRAPLINE).is_empty()
}
