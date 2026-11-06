//! On/off settings: line numbers, vim mode, terminal auto-close.

use gpui::App;
use xenon_settings::TerminalAutoClose;

use super::{change, get};

pub fn show_line_numbers(cx: &App) -> bool {
    get(cx).show_line_numbers
}

pub fn toggle_line_numbers(cx: &mut App) {
    change(cx, |s| s.show_line_numbers = !s.show_line_numbers);
}

pub fn vim_mode(cx: &App) -> bool {
    get(cx).vim_mode
}

pub fn toggle_vim_mode(cx: &mut App) {
    change(cx, |s| s.vim_mode = !s.vim_mode);
}

pub fn terminal_auto_close(cx: &App) -> TerminalAutoClose {
    get(cx).terminal_auto_close
}
