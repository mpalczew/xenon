use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape};

use xenon_term::Numbering;

use crate::look::{Look, dim};
use crate::preload::Want;
use crate::remote::{
    Cell, CursorShapeWire, CursorWire, Rendered, Rgb, Style, WIDE_SPACER, style_flags,
};

/// Render the lines of `want` the phone lacks (`holds` says which it has) and the cursor.
pub(crate) fn render<L: EventListener>(
    term: &Term<L>,
    at: Numbering,
    want: &Want,
    holds: impl Fn(u64) -> bool,
    look: &Look,
) -> Rendered {
    let cols = term.columns();
    let grid = term.grid();
    let mut links = HashMap::new();
    let lines = want
        .range
        .clone()
        .filter(|n| *n >= at.oldest && *n < at.bottom() && !holds(*n))
        .map(|n| {
            let row = &grid[Line((n as i64 - at.live as i64) as i32)];
            let cells = (0..cols)
                .map(|c| cell(term, &row[Column(c)], look, &mut links))
                .collect();
            (n, cells)
        })
        .collect();
    Rendered {
        at,
        range: want.range.clone(),
        anchor: want.anchor,
        lines,
        cursor: cursor(term),
        links,
    }
}

fn cursor<L: EventListener>(term: &Term<L>) -> Option<CursorWire> {
    if !term.mode().contains(TermMode::SHOW_CURSOR) {
        return None;
    }
    let point = term.grid().cursor.point;
    let (r, c) = (point.line.0, point.column.0);
    if r < 0 || r >= term.screen_lines() as i32 {
        return None;
    }
    Some(CursorWire {
        r: r as u16,
        c: c.min(term.columns() - 1) as u16,
        s: match term.cursor_style().shape {
            CursorShape::Beam => CursorShapeWire::Bar,
            CursorShape::Underline => CursorShapeWire::Underline,
            _ => CursorShapeWire::Block,
        },
    })
}

fn resolve<L: EventListener>(term: &Term<L>, color: Color, look: &Look) -> Option<Rgb> {
    match color {
        Color::Spec(rgb) => Some(Rgb(rgb.r, rgb.g, rgb.b)),
        Color::Indexed(n) => Some(palette(term, usize::from(n), look)),
        Color::Named(named) => match named as usize {
            n @ 0..=15 => Some(palette(term, n, look)),
            n @ 259..=266 => Some(dim(palette(term, n - 259, look), look.bg)),
            _ => None,
        },
    }
}

fn palette<L: EventListener>(term: &Term<L>, index: usize, look: &Look) -> Rgb {
    match term.colors()[index] {
        Some(rgb) => Rgb(rgb.r, rgb.g, rgb.b),
        None => look.indexed(index as u8),
    }
}

fn cell<L: EventListener>(
    term: &Term<L>,
    c: &GridCell,
    look: &Look,
    links: &mut HashMap<u64, String>,
) -> Cell {
    if c.flags.contains(Flags::WIDE_CHAR_SPACER) {
        return Cell {
            ch: WIDE_SPACER,
            style: Style::default(),
        };
    }
    let (mut fg, mut bg) = (resolve(term, c.fg, look), resolve(term, c.bg, look));
    if c.flags.contains(Flags::INVERSE) {
        let (f, b) = (fg.unwrap_or(look.fg), bg.unwrap_or(look.bg));
        (fg, bg) = (Some(b), Some(f));
    }
    if c.flags.contains(Flags::DIM) {
        let base = fg.unwrap_or(look.fg);
        fg = Some(dim(base, bg.unwrap_or(look.bg)));
    }
    let mut flags = 0;
    for (flag, bit) in [
        (Flags::BOLD, style_flags::BOLD),
        (Flags::ITALIC, style_flags::ITALIC),
        (Flags::ALL_UNDERLINES, style_flags::UNDERLINE),
        (Flags::STRIKEOUT, style_flags::STRIKE),
    ] {
        if c.flags.intersects(flag) {
            flags |= bit;
        }
    }
    let link = c.hyperlink().map_or(0, |h| {
        let mut hasher = DefaultHasher::new();
        h.uri().hash(&mut hasher);
        let key = hasher.finish().max(1);
        links.entry(key).or_insert_with(|| h.uri().to_string());
        key
    });
    let blank = c.flags.contains(Flags::HIDDEN)
        || c.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
        || c.c.is_control();
    Cell {
        ch: if blank { ' ' } else { c.c },
        style: Style {
            fg,
            bg,
            flags,
            link,
        },
    }
}
