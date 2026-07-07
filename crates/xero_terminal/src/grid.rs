//! Grid rendering: turn a terminal's `Content` snapshot into painted quads and
//! shaped lines. Layout logic is adapted from zed's `terminal_element.rs`
//! (`layout_grid`), GPL-3.0-or-later; see ATTRIBUTION.md.

use gpui::{
    Bounds, Entity, Font, FontFeatures, Hsla, Pixels, Point as GpuiPoint, ShapedLine, SharedString,
    Size, TextAlign, TextRun, Window, fill, point, px,
};
use terminal::{Terminal, TerminalBounds};
use theme::{ActiveTheme, Theme};

use crate::color::convert_color;

/// A line ready to paint at a pixel offset, plus its per-cell background quads.
pub struct GridLine {
    pub origin: GpuiPoint<Pixels>,
    pub line: ShapedLine,
    pub backgrounds: Vec<(Bounds<Pixels>, Hsla)>,
}

/// The shaped grid plus the cursor and selection rectangles to overlay.
pub struct GridLayout {
    pub lines: Vec<GridLine>,
    pub cursor: Option<Bounds<Pixels>>,
    pub selection: Vec<Bounds<Pixels>>,
}

/// The monospace font used for the grid. Menlo is always present on macOS;
/// ligatures are disabled so cell advances stay uniform.
pub fn terminal_font() -> Font {
    let mut font = gpui::font("Menlo");
    font.features = FontFeatures::disable_ligatures();
    font
}

/// Cell advance width for the terminal font at `font_size` (advance of 'm').
pub fn cell_width(window: &Window, font: &Font, font_size: Pixels) -> Pixels {
    let font_id = window.text_system().resolve_font(font);
    window
        .text_system()
        .advance(font_id, font_size, 'm')
        .map(|size| size.width)
        .unwrap_or(font_size * 0.6)
}

/// Resize the terminal to `bounds`, pull a fresh snapshot, and shape every row.
pub fn layout(
    terminal: &Entity<Terminal>,
    bounds: Bounds<Pixels>,
    font: &Font,
    font_size: Pixels,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut gpui::App,
) -> GridLayout {
    let cell_w = cell_width(window, font, font_size);
    let dimensions = TerminalBounds::new(line_height, cell_w, bounds);
    terminal.update(cx, |terminal, cx| {
        terminal.set_size(dimensions);
        terminal.sync(window, cx);
    });

    let theme = cx.theme().clone();
    let content = terminal.read(cx).last_content().clone();
    // Scrollback rows carry negative alacritty line numbers; the display row is
    // `point.line + display_offset` (matches zed's terminal_element). Without
    // this, scrolling renders history off the top and the viewport empties out.
    let offset = content.display_offset as i32;
    let rows = content.terminal_bounds.num_lines() as i32;
    let cursor = cursor_bounds(&content, bounds.origin, cell_w, line_height, offset, rows);
    let selection = selection_rects(&content, bounds.origin, cell_w, line_height, offset);
    let lines = shape_rows(
        &content, bounds.origin, cell_w, line_height, offset, font, font_size, &theme, window,
    );
    GridLayout { lines, cursor, selection }
}

/// Highlight rectangles for the active selection, one per display row.
fn selection_rects(
    content: &terminal::Content,
    origin: GpuiPoint<Pixels>,
    cell_w: Pixels,
    line_height: Pixels,
    offset: i32,
) -> Vec<Bounds<Pixels>> {
    let Some(selection) = content.selection else {
        return Vec::new();
    };
    let range = selection.point_range();
    let (start, end) = (range.start(), range.end());
    let num_cols = content.terminal_bounds.num_columns();
    let mut rects = Vec::new();
    for line in start.line..=end.line {
        let y = origin.y + line_height * ((line + offset) as f32);
        let (first, last) = if start.line == end.line {
            (start.column, end.column + 1)
        } else if line == start.line {
            (start.column, num_cols)
        } else if line == end.line {
            (0, end.column + 1)
        } else {
            (0, num_cols)
        };
        let last = last.min(num_cols);
        if last > first {
            let x = origin.x + cell_w * (first as f32);
            let width = cell_w * ((last - first) as f32);
            rects.push(Bounds::new(point(x, y), Size { width, height: line_height }));
        }
    }
    rects
}

/// The cell rectangle the cursor occupies, if visible and not hidden.
fn cursor_bounds(
    content: &terminal::Content,
    origin: GpuiPoint<Pixels>,
    cell_w: Pixels,
    line_height: Pixels,
    offset: i32,
    rows: i32,
) -> Option<Bounds<Pixels>> {
    let cursor = &content.cursor;
    if matches!(cursor.shape, terminal::CursorShape::Hidden) {
        return None;
    }
    let display_line = cursor.point.line + offset;
    if display_line < 0 || display_line >= rows {
        return None; // Scrolled out of the viewport.
    }
    let cell_origin = point(
        origin.x + cell_w * (cursor.point.column as f32),
        origin.y + line_height * (display_line as f32),
    );
    Some(Bounds::new(cell_origin, Size { width: cell_w, height: line_height }))
}

/// Group the flat cell list into rows and shape each into a `ShapedLine`.
fn shape_rows(
    content: &terminal::Content,
    origin: GpuiPoint<Pixels>,
    cell_w: Pixels,
    line_height: Pixels,
    offset: i32,
    font: &Font,
    font_size: Pixels,
    theme: &Theme,
    window: &mut Window,
) -> Vec<GridLine> {
    let mut lines = Vec::new();
    let mut row: Option<Row> = None;
    for indexed in &content.cells {
        match &mut row {
            Some(r) if r.line == indexed.point.line => r.push(indexed, theme, font),
            _ => {
                if let Some(r) = row.take() {
                    lines.push(r.finish(origin, cell_w, line_height, offset, font_size, window));
                }
                let mut r = Row::new(indexed.point.line);
                r.push(indexed, theme, font);
                row = Some(r);
            }
        }
    }
    if let Some(r) = row {
        lines.push(r.finish(origin, cell_w, line_height, offset, font_size, window));
    }
    lines
}

/// Accumulates one terminal row into a string + text runs + background quads.
struct Row {
    line: i32,
    text: String,
    runs: Vec<TextRun>,
    backgrounds: Vec<(i32, Hsla)>,
}

impl Row {
    fn new(line: i32) -> Self {
        Row { line, text: String::new(), runs: Vec::new(), backgrounds: Vec::new() }
    }

    fn push(&mut self, indexed: &terminal::IndexedCell, theme: &Theme, font: &Font) {
        let cell = &indexed.cell;
        if cell.is_wide_char_spacer() {
            return;
        }
        let (mut fg, mut bg) = (cell.foreground(), cell.background());
        if cell.is_inverse() {
            std::mem::swap(&mut fg, &mut bg);
        }
        let column = indexed.point.column as i32;
        if !terminal::is_default_background_color(bg) {
            self.backgrounds.push((column, convert_color(&bg, theme)));
        }
        let ch = cell.character();
        self.text.push(ch);
        self.runs.push(TextRun {
            len: ch.len_utf8(),
            color: convert_color(&fg, theme),
            background_color: None,
            font: font.clone(),
            underline: None,
            strikethrough: None,
        });
    }

    fn finish(
        self,
        origin: GpuiPoint<Pixels>,
        cell_w: Pixels,
        line_height: Pixels,
        offset: i32,
        font_size: Pixels,
        window: &mut Window,
    ) -> GridLine {
        let y = origin.y + line_height * ((self.line + offset) as f32);
        let line = window.text_system().shape_line(
            SharedString::from(self.text),
            font_size,
            &self.runs,
            None,
        );
        let backgrounds = self
            .backgrounds
            .into_iter()
            .map(|(col, color)| {
                let cell_origin = point(origin.x + cell_w * (col as f32), y);
                (Bounds::new(cell_origin, Size { width: cell_w, height: line_height }), color)
            })
            .collect();
        GridLine { origin: point(origin.x, y), line, backgrounds }
    }
}

/// Paint cell backgrounds, the selection, the cursor, then the glyphs on top.
pub fn paint(layout: &GridLayout, line_height: Pixels, window: &mut Window, cx: &mut gpui::App) {
    let players = cx.theme().players().local();
    let (cursor_color, selection_color) = (players.cursor, players.selection);
    for grid_line in &layout.lines {
        for (bounds, color) in &grid_line.backgrounds {
            window.paint_quad(fill(*bounds, *color));
        }
    }
    for rect in &layout.selection {
        window.paint_quad(fill(*rect, selection_color));
    }
    if let Some(cursor) = layout.cursor {
        window.paint_quad(fill(cursor, cursor_color));
    }
    for grid_line in &layout.lines {
        let _ = grid_line
            .line
            .paint(grid_line.origin, line_height, TextAlign::Left, None, window, cx);
    }
}

/// Line height in pixels for a given font size and multiplier.
pub fn line_height(font_size: Pixels, multiplier: f32) -> Pixels {
    px(f32::from(font_size) * multiplier)
}
