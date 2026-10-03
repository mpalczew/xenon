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
pub const WIDE_SPACER: char = '\0';

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CursorShapeWire;

    fn screen(rows: &[&str], cols: u16) -> Screen {
        Screen {
            cols,
            rows: rows
                .iter()
                .map(|r| {
                    let mut cells: Vec<Cell> = r
                        .chars()
                        .map(|ch| Cell {
                            ch,
                            style: Style::default(),
                        })
                        .collect();
                    cells.resize(cols as usize, Cell::default());
                    cells
                })
                .collect(),
            cursor: Some(CursorWire {
                r: 0,
                c: 0,
                s: CursorShapeWire::Block,
            }),
            scrolled: false,
        }
    }

    fn frame(msgs: &[ServerMsg]) -> (bool, Vec<LineWire>) {
        msgs.iter()
            .find_map(|m| match m {
                ServerMsg::Frame { full, lines, .. } => Some((*full, lines.clone())),
                _ => None,
            })
            .expect("frame")
    }

    #[test]
    fn first_frame_is_full_and_declares_styles() {
        let mut enc = FrameEncoder::default();
        let msgs = enc.encode(&screen(&["hi", ""], 4));
        assert!(matches!(msgs[0], ServerMsg::Styles { .. }));
        let (full, lines) = frame(&msgs);
        assert!(full);
        assert_eq!(lines, vec![(0, vec![("hi".into(), 0)])]);
    }

    #[test]
    fn unchanged_screen_sends_nothing() {
        let mut enc = FrameEncoder::default();
        enc.encode(&screen(&["hi", "yo"], 4));
        assert!(enc.encode(&screen(&["hi", "yo"], 4)).is_empty());
    }

    #[test]
    fn scrolled_change_sends_a_frame() {
        let mut enc = FrameEncoder::default();
        let mut screen = screen(&["hi"], 4);
        enc.encode(&screen);
        screen.scrolled = true;
        let msgs = enc.encode(&screen);
        let ServerMsg::Frame {
            scrolled, lines, ..
        } = &msgs[0]
        else {
            panic!("expected a frame");
        };
        assert!(*scrolled);
        assert!(lines.is_empty());
    }

    #[test]
    fn patch_sends_only_changed_rows() {
        let mut enc = FrameEncoder::default();
        enc.encode(&screen(&["a", "b", "c"], 4));
        let (full, lines) = frame(&enc.encode(&screen(&["a", "B", "c"], 4)));
        assert!(!full);
        assert_eq!(lines, vec![(1, vec![("B".into(), 0)])]);
    }

    #[test]
    fn resize_forces_full_frame() {
        let mut enc = FrameEncoder::default();
        enc.encode(&screen(&["a"], 4));
        let (full, _) = frame(&enc.encode(&screen(&["a"], 5)));
        assert!(full);
    }

    #[test]
    fn cursor_move_alone_sends_empty_patch() {
        let mut enc = FrameEncoder::default();
        let mut s = screen(&["a"], 4);
        enc.encode(&s);
        s.cursor = Some(CursorWire {
            r: 0,
            c: 1,
            s: CursorShapeWire::Block,
        });
        let (full, lines) = frame(&enc.encode(&s));
        assert!(!full);
        assert!(lines.is_empty());
    }

    #[test]
    fn styles_split_spans_and_intern_once() {
        let red = Style {
            fg: Some(Rgb(255, 0, 0)),
            ..Style::default()
        };
        let mut s = screen(&["abcd"], 4);
        s.rows[0][2].style = red;
        s.rows[0][3].style = red;
        let mut enc = FrameEncoder::default();
        let msgs = enc.encode(&s);
        let ServerMsg::Styles { add } = &msgs[0] else {
            panic!("styles first")
        };
        assert_eq!(add.len(), 2);
        assert_eq!(add[&1].fg.as_deref(), Some("#ff0000"));
        let (_, lines) = frame(&msgs);
        assert_eq!(lines[0].1, vec![("ab".into(), 0), ("cd".into(), 1)]);
        // Unchanged screen: nothing to send.
        assert!(enc.encode(&s).is_empty());
        // A forced full frame re-declares styles (the old ones may be lost).
        enc.force_full();
        assert!(matches!(enc.encode(&s)[0], ServerMsg::Styles { .. }));
    }

    #[test]
    fn wide_spacer_sends_no_text() {
        let mut s = screen(&["好x"], 3);
        s.rows[0][1].ch = WIDE_SPACER;
        s.rows[0][2].ch = 'x';
        let mut enc = FrameEncoder::default();
        let (_, lines) = frame(&enc.encode(&s));
        assert_eq!(lines[0].1, vec![("好x".into(), 0)]);
    }

    #[test]
    fn trailing_blank_with_background_is_kept() {
        let mut s = screen(&["a"], 3);
        s.rows[0][2].style.bg = Some(Rgb(1, 2, 3));
        let mut enc = FrameEncoder::default();
        let (_, lines) = frame(&enc.encode(&s));
        assert_eq!(lines[0].1.len(), 2);
        assert_eq!(lines[0].1[0].0, "a ");
    }
}
