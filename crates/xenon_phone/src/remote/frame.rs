use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

use xenon_term::Numbering;

use crate::remote::protocol::{CursorWire, ServerMsg, SpanWire, StyleWire, style_flags};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub flags: u8,
    pub link: u64,
}

impl Style {
    fn blank_is_invisible(self) -> bool {
        self.bg.is_none() && self.flags & (style_flags::UNDERLINE | style_flags::STRIKE) == 0
    }

    fn wire(self, links: &HashMap<u64, String>) -> StyleWire {
        StyleWire {
            fg: self.fg.map(Rgb::hex),
            bg: self.bg.map(Rgb::hex),
            f: self.flags,
            h: links.get(&self.link).cloned(),
        }
    }
}

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

/// Rendered lines for one tick: the lines of `range` the phone lacks, plus every live
/// screen line in it.
#[derive(Clone)]
pub struct Snapshot {
    pub at: Numbering,
    pub range: Range<u64>,
    pub anchor: Option<u64>,
    pub lines: Vec<(u64, Vec<Cell>)>,
    pub cursor: Option<CursorWire>,
    pub links: HashMap<u64, String>,
}

/// What the phone holds: exactly `range`, with a hash of each line's spans.
struct Held {
    at: Numbering,
    range: Range<u64>,
    hashes: VecDeque<Option<u64>>,
    cursor: Option<CursorWire>,
}

impl Held {
    fn new(at: Numbering, start: u64) -> Self {
        Self {
            at,
            range: start..start,
            hashes: VecDeque::new(),
            cursor: None,
        }
    }

    /// Cut or extend to `to`, keeping the hashes of lines in both.
    fn reshape(&mut self, to: &Range<u64>) {
        if to.start >= self.range.end || to.end <= self.range.start {
            self.hashes.clear();
            self.range = to.start..to.start;
        }
        while self.range.start < to.start {
            self.hashes.pop_front();
            self.range.start += 1;
        }
        while self.range.end > to.end {
            self.hashes.pop_back();
            self.range.end -= 1;
        }
        while self.range.start > to.start {
            self.hashes.push_front(None);
            self.range.start -= 1;
        }
        while self.range.end < to.end {
            self.hashes.push_back(None);
            self.range.end += 1;
        }
    }

    fn slot(&mut self, n: u64) -> Option<&mut Option<u64>> {
        let i = n.checked_sub(self.range.start)?;
        self.hashes.get_mut(usize::try_from(i).ok()?)
    }
}

#[derive(Default)]
pub struct WindowEncoder {
    styles: HashMap<Style, u32>,
    held: Option<Held>,
}

impl WindowEncoder {
    pub fn force_full(&mut self) {
        self.held = None;
        self.styles.clear();
    }

    /// The numbering and range the phone was last sent.
    pub fn held(&self) -> Option<(Numbering, Range<u64>)> {
        self.held.as_ref().map(|h| (h.at, h.range.clone()))
    }

    /// True for a history line the phone already has: it was history when last sent, so
    /// it can't have changed since, and needs no rendering.
    pub fn holds_history(&self, n: u64, at: Numbering) -> bool {
        self.held.as_ref().is_some_and(|h| {
            h.at.epoch == at.epoch
                && n < h.at.live
                && h.range.contains(&n)
                && h.hashes[(n - h.range.start) as usize].is_some()
        })
    }

    pub fn encode(&mut self, snap: &Snapshot) -> Vec<ServerMsg> {
        let at = snap.at;
        let full = self
            .held
            .as_ref()
            .is_none_or(|h| h.at.epoch != at.epoch || h.at.alt != at.alt || h.at.cols != at.cols);
        let mut held = match self.held.take() {
            Some(h) if !full => h,
            _ => Held::new(at, snap.range.start),
        };
        let reshaped = held.range != snap.range;
        held.reshape(&snap.range);
        let mut new_styles = BTreeMap::new();
        let mut set = Vec::new();
        for (n, cells) in &snap.lines {
            let spans = self.encode_row(cells, &snap.links, &mut new_styles);
            let hash = hash_of(&spans);
            let Some(slot) = held.slot(*n) else { continue };
            // An unheld line is blank on the phone, so a blank one needs no sending.
            let blank_and_new = slot.is_none() && spans.is_empty();
            if *slot != Some(hash) && !blank_and_new {
                set.push((*n, spans));
            }
            *slot = Some(hash);
        }
        let moved = held.at != at || held.cursor != snap.cursor;
        held.at = at;
        held.cursor = snap.cursor;
        self.held = Some(held);
        if !full && !reshaped && !moved && set.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if !new_styles.is_empty() {
            out.push(ServerMsg::Styles { add: new_styles });
        }
        out.push(ServerMsg::Lines {
            epoch: at.epoch,
            full,
            cols: at.cols,
            rows: at.rows,
            alt: at.alt,
            oldest: at.oldest,
            live: at.live,
            top: snap.range.start,
            len: (snap.range.end - snap.range.start) as u32,
            set,
            cursor: snap.cursor,
            anchor: snap.anchor,
        });
        out
    }

    fn encode_row(
        &mut self,
        row: &[Cell],
        links: &HashMap<u64, String>,
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
                let id = self.intern(cell.style, links, new_styles);
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

    fn intern(
        &mut self,
        style: Style,
        links: &HashMap<u64, String>,
        new_styles: &mut BTreeMap<u32, StyleWire>,
    ) -> u32 {
        if let Some(id) = self.styles.get(&style) {
            return *id;
        }
        let id = self.styles.len() as u32;
        self.styles.insert(style, id);
        new_styles.insert(id, style.wire(links));
        id
    }
}

fn hash_of(spans: &[SpanWire]) -> u64 {
    let mut hasher = DefaultHasher::new();
    spans.hash(&mut hasher);
    hasher.finish()
}
