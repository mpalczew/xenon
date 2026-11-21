//! SGR and OSC 8 state while writing cells.

use std::fmt::Write as _;

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, NamedColor};

/// What has been written so far: the SGR state and the open link.
pub struct Pen {
    fg: Option<Color>,
    bg: Option<Color>,
    ul: Option<Color>,
    flags: Flags,
    link: Option<(String, String)>,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            ul: None,
            flags: Flags::empty(),
            link: None,
        }
    }
}

const ATTRS: [(Flags, &str); 11] = [
    (Flags::BOLD, "1"),
    (Flags::DIM, "2"),
    (Flags::ITALIC, "3"),
    (Flags::UNDERLINE, "4"),
    (Flags::DOUBLE_UNDERLINE, "4:2"),
    (Flags::UNDERCURL, "4:3"),
    (Flags::DOTTED_UNDERLINE, "4:4"),
    (Flags::DASHED_UNDERLINE, "4:5"),
    (Flags::INVERSE, "7"),
    (Flags::HIDDEN, "8"),
    (Flags::STRIKEOUT, "9"),
];

impl Pen {
    pub fn reset(&mut self, out: &mut String) {
        if self.fg.is_some() || self.bg.is_some() || self.ul.is_some() || !self.flags.is_empty() {
            out.push_str("\x1b[0m");
        }
        self.fg = None;
        self.bg = None;
        self.ul = None;
        self.flags = Flags::empty();
    }

    pub fn link(&mut self, out: &mut String, link: Option<(String, String)>) {
        if self.link == link {
            return;
        }
        match &link {
            Some((id, uri)) => {
                let id = if id.ends_with("_alacritty") {
                    String::new()
                } else {
                    format!("id={}", clean(id))
                };
                let _ = write!(out, "\x1b]8;{id};{}\x1b\\", clean(uri));
            }
            None => out.push_str("\x1b]8;;\x1b\\"),
        }
        self.link = link;
    }

    /// Set the pen to a cell's attributes (no text).
    pub fn apply(&mut self, out: &mut String, cell: &Cell) {
        let flags = cell.flags
            & (Flags::ALL_UNDERLINES
                | Flags::BOLD
                | Flags::DIM
                | Flags::ITALIC
                | Flags::INVERSE
                | Flags::HIDDEN
                | Flags::STRIKEOUT);
        let fg = non_default(cell.fg, NamedColor::Foreground);
        let bg = non_default(cell.bg, NamedColor::Background);
        let ul = cell.underline_color();
        if (fg, bg, ul, flags) == (self.fg, self.bg, self.ul, self.flags) {
            return;
        }
        // One full SGR per change keeps the output simple and correct.
        let mut sgr = String::from("0");
        for (flag, code) in ATTRS {
            if flags.contains(flag) {
                let _ = write!(sgr, ";{code}");
            }
        }
        color(&mut sgr, fg, 30, 38);
        color(&mut sgr, bg, 40, 48);
        match ul {
            Some(Color::Spec(rgb)) => {
                let _ = write!(sgr, ";58;2;{};{};{}", rgb.r, rgb.g, rgb.b);
            }
            Some(Color::Indexed(n)) => {
                let _ = write!(sgr, ";58;5;{n}");
            }
            Some(Color::Named(n)) => {
                let _ = write!(sgr, ";58;5;{}", n as usize & 0xff);
            }
            None => {}
        }
        let _ = write!(out, "\x1b[{sgr}m");
        (self.fg, self.bg, self.ul, self.flags) = (fg, bg, ul, flags);
    }

    pub fn cell(&mut self, out: &mut String, cell: &Cell) {
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            return;
        }
        self.apply(out, cell);
        let link = cell.hyperlink();
        self.link(out, link.map(|l| (l.id().to_owned(), l.uri().to_owned())));
        out.push(if cell.c.is_control() { ' ' } else { cell.c });
        if let Some(extra) = cell.zerowidth() {
            out.extend(extra.iter().filter(|c| !c.is_control()));
        }
    }
}

fn non_default(color: Color, default: NamedColor) -> Option<Color> {
    (color != Color::Named(default)).then_some(color)
}

fn color(sgr: &mut String, color: Option<Color>, base: usize, extended: u8) {
    let _ = match color {
        None => return,
        Some(Color::Spec(rgb)) => write!(sgr, ";{extended};2;{};{};{}", rgb.r, rgb.g, rgb.b),
        Some(Color::Indexed(n)) => write!(sgr, ";{extended};5;{n}"),
        Some(Color::Named(n)) => match n as usize {
            0..=7 => write!(sgr, ";{}", base + n as usize),
            8..=15 => write!(sgr, ";{}", base + 52 + n as usize),
            _ => return,
        },
    };
}

/// Drop control characters so they can't end an escape sequence early.
pub fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}
