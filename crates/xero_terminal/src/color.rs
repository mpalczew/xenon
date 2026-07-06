// Adapted from zed's crates/terminal_view/src/terminal_element.rs (`convert_color`),
// GPL-3.0-or-later. See ATTRIBUTION.md. The `Spec`/`Indexed` arms delegate to
// `terminal`'s public helpers; only the `Named` mapping is reproduced here.

use terminal::{Color, NamedColor};
use theme::Theme;

use gpui::Hsla;

/// Resolve an alacritty terminal color against the active theme's ANSI palette.
pub fn convert_color(color: &Color, theme: &Theme) -> Hsla {
    match color {
        Color::Named(named) => named_color(*named, theme),
        Color::Spec(rgb) => terminal::rgba_color(rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => terminal::get_color_at_index(*i as usize, theme),
    }
}

/// Named ANSI colors, mapped onto the theme's `terminal_ansi_*` fields.
fn named_color(named: NamedColor, theme: &Theme) -> Hsla {
    let colors = theme.colors();
    match named {
        NamedColor::Black => colors.terminal_ansi_black,
        NamedColor::Red => colors.terminal_ansi_red,
        NamedColor::Green => colors.terminal_ansi_green,
        NamedColor::Yellow => colors.terminal_ansi_yellow,
        NamedColor::Blue => colors.terminal_ansi_blue,
        NamedColor::Magenta => colors.terminal_ansi_magenta,
        NamedColor::Cyan => colors.terminal_ansi_cyan,
        NamedColor::White => colors.terminal_ansi_white,
        NamedColor::BrightBlack => colors.terminal_ansi_bright_black,
        NamedColor::BrightRed => colors.terminal_ansi_bright_red,
        NamedColor::BrightGreen => colors.terminal_ansi_bright_green,
        NamedColor::BrightYellow => colors.terminal_ansi_bright_yellow,
        NamedColor::BrightBlue => colors.terminal_ansi_bright_blue,
        NamedColor::BrightMagenta => colors.terminal_ansi_bright_magenta,
        NamedColor::BrightCyan => colors.terminal_ansi_bright_cyan,
        NamedColor::BrightWhite => colors.terminal_ansi_bright_white,
        NamedColor::Foreground => colors.terminal_foreground,
        NamedColor::Background => colors.terminal_ansi_background,
        NamedColor::Cursor => theme.players().local().cursor,
        NamedColor::DimBlack => colors.terminal_ansi_dim_black,
        NamedColor::DimRed => colors.terminal_ansi_dim_red,
        NamedColor::DimGreen => colors.terminal_ansi_dim_green,
        NamedColor::DimYellow => colors.terminal_ansi_dim_yellow,
        NamedColor::DimBlue => colors.terminal_ansi_dim_blue,
        NamedColor::DimMagenta => colors.terminal_ansi_dim_magenta,
        NamedColor::DimCyan => colors.terminal_ansi_dim_cyan,
        NamedColor::DimWhite => colors.terminal_ansi_dim_white,
        NamedColor::BrightForeground => colors.terminal_bright_foreground,
        NamedColor::DimForeground => colors.terminal_dim_foreground,
    }
}
