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

/// One task in `GET /api/workspaces/{id}/tasks`. `command` is the line that
/// will run (variables expanded where Xenon can, `${input:...}` left as
/// written), so a label cannot hide it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub command: String,
    /// `build` or `test`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
    /// Why it cannot run in Xenon; such tasks are listed but not runnable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported: Option<String>,
}

/// A value that must not reach logs: serializes as a plain string, prints
/// as `Secret(..)`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskInputKind {
    Text,
    /// Masked on the phone; never stored there.
    Password,
    Pick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOptionWire {
    pub label: String,
    pub value: String,
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
    /// Output cadence for this connection; responsive unless explicitly enabled.
    #[serde(rename_all = "camelCase")]
    Bandwidth {
        low_bandwidth: bool,
    },
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
    /// Left click on a live-grid cell. The Mac reports it only while the
    /// program has mouse mode on.
    Click {
        col: u16,
        row: u16,
    },
    /// Phone viewport in cells; the Mac sizes the PTY to it while idle locally.
    Fit {
        cols: u16,
        rows: u16,
    },
    /// Liveness probe; answered with `pong` by the server thread itself.
    Ping,
    /// Run a task of that workspace on the Mac, exactly as the Mac's Run Task
    /// picker would. Questions come back as `taskInput`.
    #[serde(rename_all = "camelCase")]
    RunTask {
        workspace_id: String,
        label: String,
    },
    /// Answer to `taskInput`; no `value` cancels the run.
    #[serde(rename_all = "camelCase")]
    TaskInput {
        token: u64,
        #[serde(default)]
        value: Option<Secret>,
    },
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
    /// A run started from this phone needs an input answered (`taskInput`).
    #[serde(rename_all = "camelCase")]
    TaskInput {
        token: u64,
        id: String,
        description: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        kind: TaskInputKind,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        options: Vec<TaskOptionWire>,
    },
    /// The task's terminal exists: attach to `tab_id` like any terminal.
    #[serde(rename_all = "camelCase")]
    TaskStarted {
        workspace_id: String,
        label: String,
        tab_id: u64,
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
        for low_bandwidth in [false, true] {
            let msg = ClientMsg::Bandwidth { low_bandwidth };
            let json = format!(r#"{{"t":"bandwidth","lowBandwidth":{low_bandwidth}}}"#);
            assert_eq!(serde_json::from_str::<ClientMsg>(&json).unwrap(), msg);
            assert_eq!(serde_json::to_string(&msg).unwrap(), json);
        }
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
        let click: ClientMsg = serde_json::from_str(r#"{"t":"click","col":3,"row":4}"#).unwrap();
        assert_eq!(click, ClientMsg::Click { col: 3, row: 4 });
        let page: ClientMsg = serde_json::from_str(r#"{"t":"key","key":"page-up"}"#).unwrap();
        assert_eq!(
            page,
            ClientMsg::Key {
                key: NamedKey::PageUp
            }
        );
    }

    #[test]
    fn task_messages_roundtrip_and_hide_secrets() {
        let run: ClientMsg =
            serde_json::from_str(r#"{"t":"runTask","workspaceId":"w","label":"build"}"#).unwrap();
        assert_eq!(
            run,
            ClientMsg::RunTask {
                workspace_id: "w".into(),
                label: "build".into()
            }
        );
        let answer: ClientMsg =
            serde_json::from_str(r#"{"t":"taskInput","token":4,"value":"hunter2"}"#).unwrap();
        assert_eq!(
            answer,
            ClientMsg::TaskInput {
                token: 4,
                value: Some(Secret("hunter2".into()))
            }
        );
        assert!(!format!("{answer:?}").contains("hunter2"));
        let cancel: ClientMsg = serde_json::from_str(r#"{"t":"taskInput","token":4}"#).unwrap();
        assert_eq!(
            cancel,
            ClientMsg::TaskInput {
                token: 4,
                value: None
            }
        );
        let ask = ServerMsg::TaskInput {
            token: 4,
            id: "pw".into(),
            description: "Password".into(),
            default: None,
            kind: TaskInputKind::Password,
            options: vec![],
        };
        assert_eq!(
            serde_json::to_string(&ask).unwrap(),
            r#"{"t":"taskInput","token":4,"id":"pw","description":"Password","kind":"password"}"#
        );
        let info = TaskInfo {
            label: "b".into(),
            detail: None,
            command: "make".into(),
            group: Some("build".into()),
            default: true,
            unsupported: None,
        };
        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"label":"b","command":"make","group":"build","default":true}"#
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
}
