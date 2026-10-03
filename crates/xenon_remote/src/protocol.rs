//! JSON wire types for the mobile remote: REST lists, pairing, and the
//! WebSocket session. `page.html` is the only client; keep the two in step.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Terminal / workspace status pip, mirroring the desktop sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dot {
    Working,
    Attention,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    /// Live in this app process (terminals available without reopening).
    #[serde(default)]
    pub open: bool,
    /// Root path for display (`~` for `$HOME`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dot: Option<Dot>,
    /// Terminal tab count (0 for closed workspaces).
    #[serde(default)]
    pub terminals: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub tab_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub cwd: String,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dot: Option<Dot>,
}

/// `POST /pair`: the 6-digit code or the QR fragment secret.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    pub code: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairResponse {
    pub device_id: String,
    pub token: String,
    /// Human name of the Mac, for the "Paired" screen.
    pub host_name: String,
}

/// Special keys the phone key row sends. Resolved on the Mac through the
/// terminal's keystroke path so app-cursor mode is respected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamedKey {
    Enter,
    Esc,
    CtrlC,
    Tab,
    ShiftTab,
    Up,
    Down,
    Left,
    Right,
    Backspace,
    PageUp,
    PageDown,
    CtrlD,
    CtrlU,
}

impl NamedKey {
    /// gpui keystroke source (`Keystroke::parse`).
    pub fn keystroke(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Esc => "escape",
            Self::CtrlC => "ctrl-c",
            Self::Tab => "tab",
            Self::ShiftTab => "shift-tab",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Backspace => "backspace",
            Self::PageUp => "pageup",
            Self::PageDown => "pagedown",
            Self::CtrlD => "ctrl-d",
            Self::CtrlU => "ctrl-u",
        }
    }
}

/// Phone → Mac over `/ws`. The first message must be `hello`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMsg {
    Hello {
        token: String,
    },
    #[serde(rename_all = "camelCase")]
    Attach {
        workspace_id: String,
        tab_id: u64,
    },
    Detach,
    /// Raw text; no implicit Enter.
    Text {
        data: String,
    },
    Key {
        key: NamedKey,
    },
    /// Scrollback above the live screen (plain text, newest last).
    History,
    /// Finger or trackpad wheel, in terminal rows. Positive shows older lines.
    Wheel {
        rows: i32,
    },
    /// Jump the terminal viewport back to the live row.
    Bottom,
    /// Phone viewport in cells; the Mac sizes the PTY to it while idle locally.
    Fit {
        cols: u16,
        rows: u16,
    },
    /// Liveness probe; answered with `pong` by the server thread itself.
    Ping,
}

/// Theme colors so the page matches the Mac terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeWire {
    pub bg: String,
    pub fg: String,
    pub cursor: String,
}

/// One interned text style. Colors are `#rrggbb`; `None` = theme default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StyleWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Bit flags: see `style_flags`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub f: u8,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// `StyleWire::f` bits.
pub mod style_flags {
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
    pub const DIM: u8 = 8;
    pub const STRIKE: u8 = 16;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorShapeWire {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorWire {
    pub r: u16,
    pub c: u16,
    pub s: CursorShapeWire,
}

/// A run of text in one style: `[text, styleId]`.
pub type SpanWire = (String, u32);
/// A changed row: `[rowIndex, spans]`.
pub type LineWire = (u16, Vec<SpanWire>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClosedReason {
    TabClosed,
    Revoked,
    ServerStopping,
}

/// Mac → phone over `/ws`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ServerMsg {
    #[serde(rename_all = "camelCase")]
    Ready {
        theme: ThemeWire,
        device_id: String,
        host_name: String,
    },
    /// New interned styles, sent before the first frame that uses them.
    Styles {
        add: BTreeMap<u32, StyleWire>,
    },
    /// Screen patch. `full` = replace every row (first frame or resize).
    Frame {
        seq: u64,
        cols: u16,
        rows: u16,
        full: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<CursorWire>,
        lines: Vec<LineWire>,
        /// The terminal viewport is above its live row (shell scrollback).
        #[serde(default, skip_serializing_if = "is_false")]
        scrolled: bool,
    },
    History {
        lines: Vec<String>,
    },
    /// Status pips for every workspace, plus tabs of `workspace` (the
    /// attached one; `None` when the phone isn't attached).
    Dots {
        workspaces: BTreeMap<String, Dot>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        tabs: BTreeMap<u64, Dot>,
    },
    Closed {
        reason: ClosedReason,
    },
    Error {
        msg: String,
    },
    Pong,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_msgs_parse_from_page_shapes() {
        let hello: ClientMsg = serde_json::from_str(r#"{"t":"hello","token":"abc"}"#).unwrap();
        assert_eq!(
            hello,
            ClientMsg::Hello {
                token: "abc".into()
            }
        );
        let attach: ClientMsg =
            serde_json::from_str(r#"{"t":"attach","workspaceId":"w","tabId":7}"#).unwrap();
        assert_eq!(
            attach,
            ClientMsg::Attach {
                workspace_id: "w".into(),
                tab_id: 7
            }
        );
        let key: ClientMsg = serde_json::from_str(r#"{"t":"key","key":"shift-tab"}"#).unwrap();
        assert_eq!(
            key,
            ClientMsg::Key {
                key: NamedKey::ShiftTab
            }
        );
        let fit: ClientMsg = serde_json::from_str(r#"{"t":"fit","cols":45,"rows":30}"#).unwrap();
        assert_eq!(fit, ClientMsg::Fit { cols: 45, rows: 30 });
        let wheel: ClientMsg = serde_json::from_str(r#"{"t":"wheel","rows":-2}"#).unwrap();
        assert_eq!(wheel, ClientMsg::Wheel { rows: -2 });
        let bottom: ClientMsg = serde_json::from_str(r#"{"t":"bottom"}"#).unwrap();
        assert_eq!(bottom, ClientMsg::Bottom);
        let page: ClientMsg = serde_json::from_str(r#"{"t":"key","key":"page-up"}"#).unwrap();
        assert_eq!(
            page,
            ClientMsg::Key {
                key: NamedKey::PageUp
            }
        );
    }

    #[test]
    fn frame_serializes_compactly() {
        let msg = ServerMsg::Frame {
            seq: 3,
            cols: 4,
            rows: 1,
            full: true,
            cursor: Some(CursorWire {
                r: 0,
                c: 2,
                s: CursorShapeWire::Block,
            }),
            lines: vec![(0, vec![("ab".into(), 0)])],
            scrolled: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"t":"frame","seq":3,"cols":4,"rows":1,"full":true,"cursor":{"r":0,"c":2,"s":"block"},"lines":[[0,[["ab",0]]]]}"#
        );
    }

    #[test]
    fn default_style_serializes_empty() {
        assert_eq!(serde_json::to_string(&StyleWire::default()).unwrap(), "{}");
    }
}
