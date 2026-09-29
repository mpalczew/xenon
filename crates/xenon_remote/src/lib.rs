//! Mobile remote: an HTTP + WebSocket server so a phone can pair, list
//! workspaces, and drive a terminal. See `docs/designs/mobile-remote-v2.md`.

mod auth;
mod frame;
mod host;
mod http;
mod pairing;
mod protocol;
mod rate_limit;
mod server;
mod ws;

pub use auth::{ct_eq, hash_token, new_device_token};
pub use frame::{Cell, FrameEncoder, Rgb, Screen, Style, WIDE_SPACER};
pub use host::{ConnId, HostRequest, HostTx, Outbox, PairError};
pub use pairing::{PAIRING_TTL, PairingCode};
pub use protocol::{
    ClientMsg, ClosedReason, CursorShapeWire, CursorWire, Dot, NamedKey, PairRequest, PairResponse,
    ServerMsg, StyleWire, TerminalInfo, ThemeWire, WorkspaceInfo, style_flags,
};
pub use server::RemoteServer;
