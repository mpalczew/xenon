mod auth;
mod device_flow;
mod devices;
mod fit;
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
mod vte_html;
mod ws;

pub use devices::DeviceBook;
pub use fit::{FitGate, clamp_fit, smallest_fit};
pub use frame::{Cell, FrameEncoder, Rgb, Screen, Style};
pub use host::{ConnId, HostRequest, Outbox, PairError};
pub use keys::pty_text_bytes;
pub use labels::device_short_name;
pub use listing::{list_workspaces, parse_workspace_id, tilde, wire_dot};
pub use names::magic_dns_name;
pub use network::{Reachability, base_url, normalize_hostname, pairing_url};

pub use protocol::{
    AppearanceWire, ClientMsg, ClosedReason, CursorShapeWire, CursorWire, Dot, PairResponse,
    Secret, ServerMsg, TaskInfo, TaskInputKind, TaskOptionWire, TerminalInfo, WorkspaceInfo,
    style_flags,
};
pub use server::{RemoteServer, StartError};
pub use status::{
    DeviceRow, KEEP_AWAKE_DETAIL, KEEP_AWAKE_LABEL, RemoteStatus, ago, device_rows, effective_port,
    theme_wire,
};
pub use vte_html::{LinkRun, VteSnapshot};
