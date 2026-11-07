//! VTE's HTML dump of the visible screen → a wire `Screen`.
//!
//! `vte_terminal_get_text_format(HTML)` returns `<pre>` with one `\n` per row
//! and spans for what is not default: `<font color>`, `<b>`, `<i>`, `<u>`,
//! `<strike>`, and `<span style="background-color:...">`. It has no markup for
//! dim, hyperlinks, or the cursor, and does not mark the second column of a
//! wide glyph, so widths come from Unicode. This is the boundary format the
//! GTK frontend parses; it knows nothing of GTK.

use unicode_width::UnicodeWidthChar;

use crate::frame::{Cell, Rgb, Screen, Style, WIDE_SPACER};
use crate::protocol::{CursorWire, style_flags};

/// One dump of a VTE screen plus what the HTML does not carry.
#[derive(Clone)]
pub struct VteSnapshot {
    pub html: String,
    pub cols: u16,
    pub rows: u16,
    pub cursor: Option<CursorWire>,
    /// The viewport is above the live row.
    pub scrolled: bool,
    /// Colors equal to these are the theme default and sent as such.
    pub default_fg: Rgb,
    pub default_bg: Rgb,
}

#[derive(Clone, Copy, Default)]
struct Pen {
    fg: Option<Rgb>,
    bg: Option<Rgb>,
    flags: u8,
}

impl VteSnapshot {
    pub fn into_screen(self) -> Screen {
        let (cols, rows) = (usize::from(self.cols), usize::from(self.rows));
        let mut grid = vec![vec![Cell::default(); cols]; rows];
        let (mut row, mut col) = (0usize, 0usize);
        let mut stack: Vec<Pen> = vec![Pen::default()];
        let html = self.html.as_str();
        let html = html.strip_prefix("<pre>").unwrap_or(html);
        let mut rest = html;
        while !rest.is_empty() {
            if let Some(tag) = rest.strip_prefix('<') {
                let end = tag.find('>').unwrap_or(tag.len());
                apply_tag(&tag[..end], &mut stack);
                rest = tag.get(end + 1..).unwrap_or("");
                continue;
            }
            let (ch, used) = next_char(rest);
            rest = &rest[used..];
            if ch == '\n' {
                row += 1;
                col = 0;
                continue;
            }
            let width = ch.width().unwrap_or(0);
            if width == 0 || row >= rows {
                continue;
            }
            let pen = stack.last().copied().unwrap_or_default();
            let style = Style {
                fg: pen.fg.filter(|c| *c != self.default_fg),
                bg: pen.bg.filter(|c| *c != self.default_bg),
                flags: pen.flags,
            };
            if col + width > cols {
                continue;
            }
            grid[row][col] = Cell { ch, style };
            if width == 2 {
                grid[row][col + 1] = Cell {
                    ch: WIDE_SPACER,
                    style,
                };
            }
            col += width;
        }
        Screen {
            cols: self.cols,
            rows: grid,
            cursor: self.cursor,
            scrolled: self.scrolled,
        }
    }
}

/// The next character, decoding `&...;` entities. Returns it and the bytes used.
fn next_char(s: &str) -> (char, usize) {
    if let Some(entity) = s.strip_prefix('&')
        && let Some(end) = entity.find(';').filter(|e| *e <= 8)
    {
        let name = &entity[..end];
        let decoded = match name {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name.strip_prefix('#').and_then(|n| {
                let code = match n.strip_prefix('x') {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => n.parse().ok()?,
                };
                char::from_u32(code)
            }),
        };
        if let Some(ch) = decoded {
            return (ch, end + 2);
        }
    }
    let ch = s.chars().next().unwrap_or('\n');
    (ch, ch.len_utf8())
}

/// Open or close one tag (`inner` is the text between `<` and `>`).
fn apply_tag(inner: &str, stack: &mut Vec<Pen>) {
    if inner.starts_with('/') {
        if stack.len() > 1 {
            stack.pop();
        }
        return;
    }
    let mut pen = stack.last().copied().unwrap_or_default();
    let name = inner.split_whitespace().next().unwrap_or("");
    match name {
        "b" => pen.flags |= style_flags::BOLD,
        "i" => pen.flags |= style_flags::ITALIC,
        "u" => pen.flags |= style_flags::UNDERLINE,
        "strike" | "s" | "del" => pen.flags |= style_flags::STRIKE,
        "font" => {
            if let Some(color) = attr_color(inner, "color=\"") {
                pen.fg = Some(color);
            }
        }
        "span" => {
            if let Some(color) = attr_color(inner, "background-color:") {
                pen.bg = Some(color);
            }
            if let Some(color) = attr_color(inner, "style=\"color:") {
                pen.fg = Some(color);
            }
        }
        _ => {}
    }
    stack.push(pen);
}

/// The `#rrggbb` right after `key` in a tag.
fn attr_color(tag: &str, key: &str) -> Option<Rgb> {
    let at = tag.find(key)? + key.len();
    let hex = tag.get(at..)?.trim_start().strip_prefix('#')?.get(..6)?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}
