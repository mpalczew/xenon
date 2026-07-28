use super::*;

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Bytes to write to the PTY for typed/injected text.
/// Enter key = CR; convert LF / CRLF so inject matches a real Enter press.
pub(crate) fn pty_input_bytes(text: &str) -> Vec<u8> {
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}

/// macOS text input: only `replace_text_in_range` is needed for plain typing;
/// the rest satisfy the protocol. IME composition is not handled in v1.
impl EntityInputHandler for TerminalView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar_focused(window) {
            self.append_find_query(text, cx);
            return;
        }
        self.send_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar_focused(window) {
            self.append_find_query(new_text, cx);
            return;
        }
        self.send_text(new_text, cx);
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pty_input_bytes;

    #[test]
    fn enter_is_cr_not_lf() {
        assert_eq!(pty_input_bytes("test\n"), b"test\r");
        assert_eq!(pty_input_bytes("test\r\n"), b"test\r");
        assert_eq!(pty_input_bytes("a\nb\n"), b"a\rb\r");
        assert_eq!(pty_input_bytes("plain"), b"plain");
    }
}
