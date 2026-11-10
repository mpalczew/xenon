use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub open: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dot: Option<Dot>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported: Option<String>,
}

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
    Password,
    Pick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOptionWire {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairRequest {
    pub code: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairResponse {
    pub device_id: String,
    pub token: String,
    pub host_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamedKey {
    Enter,
    ShiftEnter,
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
    #[serde(rename_all = "camelCase")]
    Bandwidth {
        low_bandwidth: bool,
    },
    /// The page's chosen appearance; `None` follows the Mac's active theme.
    Appearance {
        appearance: Option<AppearanceWire>,
    },
    Text {
        data: String,
    },
    Key {
        key: NamedKey,
    },
    History,
    /// Finger or trackpad wheel, in terminal rows. Positive shows older lines.
    Wheel {
        rows: i32,
    },
    Bottom,
    Click {
        col: u16,
        row: u16,
    },
    Fit {
        cols: u16,
        rows: u16,
    },
    Ping,
    #[serde(rename_all = "camelCase")]
    RunTask {
        workspace_id: String,
        label: String,
    },
    #[serde(rename_all = "camelCase")]
    TaskInput {
        token: u64,
        #[serde(default)]
        value: Option<Secret>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceWire {
    Dark,
    Light,
}

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
    #[serde(default, skip_serializing_if = "is_zero")]
    pub f: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<String>,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

pub mod style_flags {
    pub(crate) const BOLD: u8 = 1;
    pub(crate) const ITALIC: u8 = 2;
    pub(crate) const UNDERLINE: u8 = 4;
    pub(crate) const STRIKE: u8 = 16;
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
pub(crate) type SpanWire = (String, u32);
/// A changed row: `[rowIndex, spans]`.
pub(crate) type LineWire = (u16, Vec<SpanWire>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClosedReason {
    TabClosed,
    Revoked,
    ServerStopping,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ServerMsg {
    #[serde(rename_all = "camelCase")]
    Ready {
        theme: ThemeWire,
        device_id: String,
        host_name: String,
    },
    Theme {
        theme: ThemeWire,
    },
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
        #[serde(default, skip_serializing_if = "is_false")]
        scrolled: bool,
    },
    History {
        lines: Vec<String>,
    },
    Dots {
        workspaces: BTreeMap<String, Dot>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        tabs: BTreeMap<u64, Dot>,
    },
    Closed {
        reason: ClosedReason,
    },
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
