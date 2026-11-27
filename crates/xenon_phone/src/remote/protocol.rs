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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklistInfo {
    pub groups: Vec<WorklistGroup>,
    /// Set when the op hit a changed item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    /// Token for `WorklistEdit::Undo` after a removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklistGroup {
    pub place: String,
    pub label: String,
    pub items: Vec<WorklistItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklistItem {
    pub key: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    pub editable: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum WorklistEdit {
    SetDone { item: String, done: bool },
    Add { place: String, title: String },
    Remove { item: String },
    Undo { token: u64 },
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
    Appearance {
        appearance: Option<AppearanceWire>,
    },
    Text {
        data: String,
    },
    Key {
        key: NamedKey,
    },
    /// Alt screen only: the primary screen scrolls natively on the phone.
    Wheel {
        rows: i32,
        col: u16,
        row: u16,
    },
    /// Where the phone is scrolled; `top` is the line at the top of its viewport.
    Scroll {
        epoch: u32,
        top: u64,
        follow: bool,
        /// Lines the phone's viewport shows; more than `rows` while it shrinks a wide terminal.
        lines: u16,
    },
    Click {
        col: u16,
        row: u16,
    },
    Fit {
        cols: u16,
        rows: u16,
        #[serde(default)]
        diag: Option<String>,
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
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
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

pub(crate) type SpanWire = (String, u32);
pub(crate) type LineWire = (u64, Vec<SpanWire>);

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
        host_name: String,
        /// The page reloads itself when this differs from the build it was served by.
        build: String,
    },
    Theme {
        theme: ThemeWire,
    },
    Styles {
        add: BTreeMap<u32, StyleWire>,
    },
    /// After applying, the phone holds exactly `[top, top + len)`; `set` is every line
    /// in it that is new or changed. `full` drops everything first.
    Lines {
        epoch: u32,
        full: bool,
        cols: u16,
        rows: u16,
        alt: bool,
        oldest: u64,
        live: u64,
        top: u64,
        len: u32,
        set: Vec<LineWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<CursorWire>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<u64>,
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
