//! Mobile remote: an HTTP + WebSocket server so a phone can pair, list
//! workspaces, and drive a terminal. See `docs/designs/mobile-remote-v2.md`.

mod auth;
mod device_flow;
mod devices;
mod fit;
mod footer;
mod frame;
mod host;
mod http;
mod keep_awake;
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

pub use auth::{ct_eq, hash_token, new_device_token};
pub use devices::DeviceBook;
pub use fit::{FitGate, LOCAL_QUIET, clamp_fit, smallest_fit};
pub use footer::RemoteFooter;
pub use frame::{Cell, FrameEncoder, Rgb, Screen, Style, WIDE_SPACER};
pub use host::{ConnId, HostRequest, HostTx, InputRoute, InputSink, Outbox, PairError};
pub use keep_awake::{Inhibitor, KEEP_AWAKE_DETAIL, KEEP_AWAKE_LABEL, should_hold};
pub use keys::pty_text_bytes;
pub use labels::{clean_label, device_short_name, unix_now};
pub use listing::{list_workspaces, parse_workspace_id, tilde, wire_dot};
pub use names::{computer_name, magic_dns_name};
pub use network::{Reachability, base_url, normalize_hostname, pairing_url};
pub use pairing::{PAIRING_TTL, PairingCode};
pub use protocol::{
    AppearanceWire, ClientMsg, ClosedReason, CursorShapeWire, CursorWire, Dot, NamedKey,
    PairRequest, PairResponse, Secret, ServerMsg, StyleWire, TaskInfo, TaskInputKind,
    TaskOptionWire, TerminalInfo, ThemeWire, WorkspaceInfo, style_flags,
};
pub use server::{RemoteServer, StartError};
pub use status::{
    DeviceRow, RemoteStatus, ago, device_rows, effective_port, phone_base_url, theme_wire,
};
pub use vte_html::VteSnapshot;
