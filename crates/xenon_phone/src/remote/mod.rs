mod auth;
mod device_flow;
mod devices;
mod frame;
mod host;
mod http;
mod keys;
mod labels;
mod listing;
mod names;
mod network;
mod pairing;
mod protocol;
mod rate_limit;
mod server;
mod status;
mod uploads;
mod ws;

pub(crate) use devices::DeviceBook;
pub(crate) use frame::{Cell, FrameEncoder, Rgb, Screen, Style, WIDE_SPACER};
pub(crate) use host::{ConnId, Outbox, PairError};
pub(crate) use keys::pty_text_bytes;
pub(crate) use labels::device_short_name;
pub(crate) use listing::{list_workspaces, parse_workspace_id, tilde, wire_dot};
pub(crate) use names::magic_dns_name;
pub use network::normalize_hostname;
pub(crate) use network::{Reachability, base_url, pairing_url};

pub(crate) use protocol::{
    AppearanceWire, ClientMsg, ClosedReason, CursorShapeWire, CursorWire, Dot, PairResponse,
    Secret, ServerMsg, TaskInfo, TaskInputKind, TaskOptionWire, TerminalInfo, ThemeWire,
    WorkspaceInfo, style_flags,
};
pub(crate) use server::{RemoteServer, StartError};
pub(crate) use status::device_rows;
pub use status::{
    DeviceRow, KEEP_AWAKE_DETAIL, KEEP_AWAKE_LABEL, RemoteStatus, ago, effective_port,
};
