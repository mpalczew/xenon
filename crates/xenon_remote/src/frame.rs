//! Screen → wire frames. Pure: the host builds a `Screen` from the terminal
//! grid; `FrameEncoder` interns styles and sends only rows that changed.

use std::collections::{BTreeMap, HashMap};

use crate::protocol::{CursorWire, LineWire, ServerMsg, SpanWire, StyleWire, style_flags};

/// 8-bit RGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Resolved cell style. `None` colors mean the theme default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub flags: u8,
}

impl Style {
    /// A blank in this style is invisible (safe to trim at line end).
    fn blank_is_invisible(self) -> bool {
        self.bg.is_none() && self.flags & (style_flags::UNDERLINE | style_flags::STRIKE) == 0
    }

    fn wire(self) -> StyleWire {
        StyleWire {
            fg: self.fg.map(Rgb::hex),
            bg: self.bg.map(Rgb::hex),
            f: self.flags,
        }
    }
}

/// `Cell::ch` for the second column of a wide glyph: occupies a cell, sends no text.
pub(crate) const WIDE_SPACER: char = '\0';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::default(),
        }
    }
}

/// One terminal screen: `rows` rows of cells plus the cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub cols: u16,
    pub rows: Vec<Vec<Cell>>,
    pub cursor: Option<CursorWire>,
    /// Terminal viewport is above the live row.
    pub scrolled: bool,
}

/// Per-connection encoder: remembers what the phone has so patches stay small.
#[derive(Default)]
pub struct FrameEncoder {
    styles: HashMap<Style, u32>,
    sent: Option<SentScreen>,
    seq: u64,
}

struct SentScreen {
    cols: u16,
    lines: Vec<Vec<SpanWire>>,
    cursor: Option<CursorWire>,
    scrolled: bool,
}

impl FrameEncoder {
    /// Next send is a full frame (reconnect, dropped patch).
    /// Also re-declares styles: a dropped `Styles` message is never lost.
    pub fn force_full(&mut self) {
        self.sent = None;
        self.styles.clear();
    }

    /// Messages to bring the phone up to `screen`; empty when nothing changed.
    pub fn encode(&mut self, screen: &Screen) -> Vec<ServerMsg> {
        let mut new_styles = BTreeMap::new();
        let lines: Vec<Vec<SpanWire>> = screen
            .rows
            .iter()
            .map(|row| self.encode_row(row, &mut new_styles))
            .collect();
        let full = self
            .sent
            .as_ref()
            .is_none_or(|s| s.cols != screen.cols || s.lines.len() != lines.len());
        let changed = changed_rows(self.sent.as_ref(), &lines, full);
        let cursor_moved = self.sent.as_ref().is_none_or(|s| s.cursor != screen.cursor);
        let scrolled_changed = self
            .sent
            .as_ref()
            .is_none_or(|s| s.scrolled != screen.scrolled);
        if !full && changed.is_empty() && !cursor_moved && !scrolled_changed {
            return Vec::new();
        }
        self.seq += 1;
        let mut out = Vec::new();
        if !new_styles.is_empty() {
            out.push(ServerMsg::Styles { add: new_styles });
        }
        out.push(ServerMsg::Frame {
            seq: self.seq,
            cols: screen.cols,
            rows: lines.len() as u16,
            full,
            cursor: screen.cursor,
            lines: changed,
            scrolled: screen.scrolled,
        });
        self.sent = Some(SentScreen {
            cols: screen.cols,
            lines,
            cursor: screen.cursor,
            scrolled: screen.scrolled,
        });
        out
    }

    fn encode_row(
        &mut self,
        row: &[Cell],
        new_styles: &mut BTreeMap<u32, StyleWire>,
    ) -> Vec<SpanWire> {
        let end = row
            .iter()
            .rposition(|c| c.ch != ' ' || !c.style.blank_is_invisible())
            .map_or(0, |i| i + 1);
        let mut spans: Vec<SpanWire> = Vec::new();
        let mut current: Option<Style> = None;
        for cell in &row[..end] {
            if current != Some(cell.style) {
                let id = self.intern(cell.style, new_styles);
                spans.push((String::new(), id));
                current = Some(cell.style);
            }
            if let Some(span) = spans.last_mut()
                && cell.ch != WIDE_SPACER
            {
                span.0.push(cell.ch);
            }
        }
        spans
    }

    fn intern(&mut self, style: Style, new_styles: &mut BTreeMap<u32, StyleWire>) -> u32 {
        if let Some(id) = self.styles.get(&style) {
            return *id;
        }
        let id = self.styles.len() as u32;
        self.styles.insert(style, id);
        new_styles.insert(id, style.wire());
        id
    }
}

fn changed_rows(sent: Option<&SentScreen>, lines: &[Vec<SpanWire>], full: bool) -> Vec<LineWire> {
    lines
        .iter()
        .enumerate()
        .filter(|(i, line)| {
            if full {
                return !line.is_empty();
            }
            sent.and_then(|s| s.lines.get(*i)) != Some(*line)
        })
        .map(|(i, line)| (i as u16, line.clone()))
        .collect()
}
