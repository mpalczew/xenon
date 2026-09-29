//! Phone remote support (Xenon-only): colored screen snapshots, plain-text
//! history, named keys, and the phone-fit size override.
//!
//! Phone fit: while a phone drives this tab, the PTY is sized to the phone.
//! Local keyboard input suspends that for `LOCAL_QUIET` so the Mac user can
//! take the terminal back; the fit re-applies once the Mac goes quiet again.

use std::time::Instant;

use gpui::Rgba;

use super::*;
use crate::color::convert_color;

/// Mac keyboard quiet time before a phone fit (re)applies.
pub const LOCAL_QUIET: Duration = Duration::from_secs(30);
/// `ScreenCell::ch` for the second column of a wide glyph (no text of its own).
pub const WIDE_SPACER: char = '\0';
/// Scrollback lines sent to a phone.
const HISTORY_LIMIT: usize = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellAttrs {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub strike: bool,
}

/// One grid cell with colors resolved against the active theme.
/// `None` colors are the theme default (phone uses its own default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenCell {
    pub ch: char,
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub attrs: CellAttrs,
}

impl Default for ScreenCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            attrs: CellAttrs::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenCursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenCursor {
    pub row: u16,
    pub col: u16,
    pub shape: ScreenCursorShape,
}

/// The visible grid as the Mac would paint it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenSnapshot {
    pub cols: u16,
    pub rows: Vec<Vec<ScreenCell>>,
    pub cursor: Option<ScreenCursor>,
}

/// A phone's requested grid and a short device name for the Mac banner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneFit {
    pub cols: u16,
    pub rows: u16,
    pub device: SharedString,
}

/// Terminal background, foreground, and cursor colors for the phone page.
pub fn remote_theme_colors(cx: &App) -> [[u8; 3]; 3] {
    let theme = cx.theme();
    let colors = theme.colors();
    [
        rgb(colors.terminal_background),
        rgb(colors.terminal_foreground),
        rgb(theme.players().local().cursor),
    ]
}

fn rgb(color: gpui::Hsla) -> [u8; 3] {
    let c: Rgba = color.into();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    [byte(c.r), byte(c.g), byte(c.b)]
}

impl TerminalView {
    /// Fresh snapshot of the visible grid (syncs the terminal first so it is
    /// current even when the Mac is not painting this tab).
    pub fn screen_snapshot(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ScreenSnapshot> {
        let State::Ready(terminal) = &self.state else {
            return None;
        };
        let terminal = terminal.clone();
        terminal.update(cx, |terminal, cx| terminal.sync(window, cx));
        let theme = cx.theme().clone();
        let content = terminal.read(cx).last_content();
        Some(snapshot_from_content(content, &theme))
    }

    /// Scrollback above the live screen as plain text, oldest first.
    pub fn history_text(&self, cx: &App) -> Vec<String> {
        let State::Ready(terminal) = &self.state else {
            return Vec::new();
        };
        let terminal = terminal.read(cx);
        let text = terminal.get_content();
        let lines: Vec<&str> = text.lines().collect();
        let history = lines.len().saturating_sub(terminal.viewport_lines());
        let start = history.saturating_sub(HISTORY_LIMIT);
        lines[start..history]
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect()
    }

    /// A key-row key (`ctrl-c`, `shift-tab`, `up`, …) through the normal
    /// keystroke path, so app-cursor mode is respected.
    pub fn send_named_key(&mut self, keystroke: &str, cx: &mut Context<Self>) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let Ok(keystroke) = gpui::Keystroke::parse(keystroke) else {
            return;
        };
        let terminal = terminal.clone();
        terminal.update(cx, |terminal, _| terminal.try_keystroke(&keystroke, false));
        self.note_interaction(cx);
        cx.notify();
    }

    /// Phone fit request (None = no phone driving). Applies unless the Mac
    /// typed into this tab within `LOCAL_QUIET`.
    pub fn set_phone_fit(&mut self, fit: Option<PhoneFit>, cx: &mut Context<Self>) {
        if self.phone_fit == fit {
            return;
        }
        self.phone_fit = fit;
        self.apply_size_override(cx);
        self.arm_phone_fit_timer(cx);
        cx.notify();
    }

    /// The grid the phone currently owns, if the fit is in effect.
    pub fn active_phone_fit(&self) -> Option<&PhoneFit> {
        let quiet = self
            .local_input_at
            .is_none_or(|at| at.elapsed() >= LOCAL_QUIET);
        self.phone_fit.as_ref().filter(|_| quiet)
    }

    /// Mac keyboard input: hand the grid back and re-arm the quiet timer.
    pub(super) fn note_local_input(&mut self, cx: &mut Context<Self>) {
        let was_active = self.active_phone_fit().is_some();
        self.local_input_at = Some(Instant::now());
        if self.phone_fit.is_none() {
            return;
        }
        if was_active {
            // Next paint lays out at the desktop size again.
            cx.notify();
        }
        self.arm_phone_fit_timer(cx);
    }

    /// While a requested fit waits out the Mac's quiet period, apply it when
    /// the period ends (a background tab never repaints to do it).
    fn arm_phone_fit_timer(&mut self, cx: &mut Context<Self>) {
        let Some(at) = self.local_input_at.filter(|_| self.phone_fit.is_some()) else {
            return;
        };
        let wait = LOCAL_QUIET.saturating_sub(at.elapsed());
        if wait.is_zero() {
            return;
        }
        self._phone_fit_timer = cx.spawn(async move |view, cx| {
            cx.background_executor().timer(wait).await;
            view.update(cx, |view, cx| {
                view.apply_size_override(cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// Resize the PTY to the active phone fit now (no paint needed).
    fn apply_size_override(&mut self, cx: &mut Context<Self>) {
        if let Some(fit) = self.active_phone_fit() {
            let (cols, rows) = (fit.cols, fit.rows);
            self.force_grid_size(cols, rows, cx);
        }
    }
}

fn snapshot_from_content(content: &terminal::Content, theme: &theme::Theme) -> ScreenSnapshot {
    let cols = content.terminal_bounds.num_columns();
    let rows = content.terminal_bounds.num_lines();
    let offset = content.display_offset as i32;
    let mut grid = vec![vec![ScreenCell::default(); cols]; rows];
    for indexed in &content.cells {
        let row = indexed.point.line + offset;
        let col = indexed.point.column;
        if row < 0 || row as usize >= rows || col >= cols {
            continue;
        }
        let mut cell = screen_cell(&indexed.cell, theme);
        if indexed.cell.is_wide_char_spacer() {
            cell.ch = WIDE_SPACER;
        }
        grid[row as usize][col] = cell;
    }
    ScreenSnapshot {
        cols: cols as u16,
        rows: grid,
        cursor: screen_cursor(&content.cursor, offset, rows, cols),
    }
}

fn screen_cell(cell: &terminal::Cell, theme: &theme::Theme) -> ScreenCell {
    let (mut fg, mut bg) = (cell.foreground(), cell.background());
    if cell.is_inverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    let default_fg = matches!(fg, terminal::Color::Named(terminal::NamedColor::Foreground));
    ScreenCell {
        ch: cell.character(),
        fg: (!default_fg).then(|| rgb(convert_color(&fg, theme))),
        bg: (!terminal::is_default_background_color(bg)).then(|| rgb(convert_color(&bg, theme))),
        attrs: CellAttrs {
            bold: cell.is_bold(),
            italic: cell.is_italic(),
            underline: cell.has_underline(),
            dim: cell.is_dim(),
            strike: cell.has_strikeout(),
        },
    }
}

fn screen_cursor(
    cursor: &terminal::Cursor,
    offset: i32,
    rows: usize,
    cols: usize,
) -> Option<ScreenCursor> {
    let shape = match cursor.shape {
        terminal::CursorShape::Hidden => return None,
        terminal::CursorShape::Block | terminal::CursorShape::HollowBlock => {
            ScreenCursorShape::Block
        }
        terminal::CursorShape::Bar => ScreenCursorShape::Bar,
        terminal::CursorShape::Underline => ScreenCursorShape::Underline,
    };
    let row = cursor.point.line + offset;
    if row < 0 || row as usize >= rows || cursor.point.column >= cols {
        return None;
    }
    Some(ScreenCursor {
        row: row as u16,
        col: cursor.point.column as u16,
        shape,
    })
}
