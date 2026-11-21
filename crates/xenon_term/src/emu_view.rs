//! The screen as the phone sees it: a `Screen` of cells at the phone's own scroll
//! position, read from the emulator.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::{Cell as GridCell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape};
use xenon_remote::{
    Cell, CursorShapeWire, CursorWire, Rgb, Screen, Style, WIDE_SPACER, style_flags,
};

use crate::emu::Emu;
use crate::phone::colors::{Look, dim};

pub(crate) fn history_size(emu: &Emu) -> usize {
    emu.term.grid().history_size()
}

/// Up to `limit` scrollback lines, oldest first, as plain text.
pub(crate) fn history_lines(emu: &Emu, limit: usize) -> Vec<String> {
    let grid = emu.term.grid();
    let count = history_size(emu).min(limit) as i32;
    (-count..0)
        .map(|line| {
            let row = &grid[Line(line)];
            let text: String = (0..usize::from(emu.size.cols()))
                .map(|c| &row[Column(c)])
                .filter(|c| {
                    !c.flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                })
                .map(|c| c.c)
                .collect();
            text.trim_end().to_string()
        })
        .collect()
}

/// What the phone should show `offset` rows above the live screen.
pub(crate) fn screen(emu: &Emu, offset: usize, look: &Look) -> Screen {
    let (cols, rows) = (usize::from(emu.size.cols()), usize::from(emu.size.rows()));
    let offset = offset.min(history_size(emu));
    let grid = emu.term.grid();
    let mut links = HashMap::new();
    let mut out = Vec::with_capacity(rows);
    for r in 0..rows {
        let row = &grid[Line(r as i32 - offset as i32)];
        out.push(
            (0..cols)
                .map(|c| cell(emu, &row[Column(c)], look, &mut links))
                .collect(),
        );
    }
    Screen {
        cols: emu.size.cols(),
        rows: out,
        cursor: cursor(emu, offset),
        scrolled: offset > 0,
        links,
    }
}

fn cursor(emu: &Emu, offset: usize) -> Option<CursorWire> {
    if offset > 0 || !emu.mode().contains(TermMode::SHOW_CURSOR) {
        return None;
    }
    let point = emu.term.grid().cursor.point;
    let (r, c) = (point.line.0, point.column.0);
    if r < 0 || r >= i32::from(emu.size.rows()) {
        return None;
    }
    Some(CursorWire {
        r: r as u16,
        c: c.min(usize::from(emu.size.cols()) - 1) as u16,
        s: match emu.term.cursor_style().shape {
            CursorShape::Beam => CursorShapeWire::Bar,
            CursorShape::Underline => CursorShapeWire::Underline,
            _ => CursorShapeWire::Block,
        },
    })
}

/// `None` is the theme's default color.
fn resolve(emu: &Emu, color: Color, look: &Look) -> Option<Rgb> {
    match color {
        Color::Spec(rgb) => Some(Rgb(rgb.r, rgb.g, rgb.b)),
        Color::Indexed(n) => Some(palette(emu, usize::from(n), look)),
        Color::Named(named) => match named as usize {
            n @ 0..=15 => Some(palette(emu, n, look)),
            n @ 259..=266 => Some(dim(palette(emu, n - 259, look), look.bg)),
            _ => None,
        },
    }
}

fn palette(emu: &Emu, index: usize, look: &Look) -> Rgb {
    match emu.term.colors()[index] {
        Some(rgb) => Rgb(rgb.r, rgb.g, rgb.b),
        None => look.indexed(index as u8),
    }
}

fn cell(emu: &Emu, c: &GridCell, look: &Look, links: &mut HashMap<u64, String>) -> Cell {
    if c.flags.contains(Flags::WIDE_CHAR_SPACER) {
        return Cell {
            ch: WIDE_SPACER,
            style: Style::default(),
        };
    }
    let (mut fg, mut bg) = (resolve(emu, c.fg, look), resolve(emu, c.bg, look));
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
