use crate::protocol::NamedKey;

/// Bytes to write to the PTY for typed or injected text. Enter is CR, so LF
/// and CRLF become CR, as a real Enter press would send.
pub fn pty_text_bytes(text: &str) -> Vec<u8> {
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}

impl NamedKey {
    /// The bytes this key sends. Arrow keys follow the application-cursor
    /// mode when the frontend can tell (`app_cursor`).
    pub fn pty_bytes(self, app_cursor: bool) -> &'static [u8] {
        let arrow =
            |normal: &'static [u8], app: &'static [u8]| if app_cursor { app } else { normal };
        match self {
            Self::Enter => b"\r",
            Self::ShiftEnter => b"\n",
            Self::Esc => b"\x1b",
            Self::CtrlC => b"\x03",
            Self::Tab => b"\t",
            Self::ShiftTab => b"\x1b[Z",
            Self::Up => arrow(b"\x1b[A", b"\x1bOA"),
            Self::Down => arrow(b"\x1b[B", b"\x1bOB"),
            Self::Right => arrow(b"\x1b[C", b"\x1bOC"),
            Self::Left => arrow(b"\x1b[D", b"\x1bOD"),
            Self::Backspace => b"\x7f",
            Self::PageUp => b"\x1b[5~",
            Self::PageDown => b"\x1b[6~",
            Self::CtrlD => b"\x04",
            Self::CtrlU => b"\x15",
        }
    }
}
