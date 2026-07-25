//! Mobile remote: local HTTP server so a phone can view a PTY viewport and type.
//!
//! Lean stack (no WebSocket): auth, list, inject, poll PNG frames.
//! See `docs/designs/mobile-pty-remote.md`.

mod auth;
mod frame;
mod host;
mod http;
mod png_frame;
mod protocol;
mod server;

pub use auth::{new_token, token_ok};
pub use frame::viewport_lines;
pub use host::{HostRequest, HostTx, ViewportSnapshot};
pub use png_frame::viewport_png;
pub use protocol::{
    AuthRequest, AuthResponse, FrameMeta, InjectRequest, TerminalInfo, WorkspaceInfo,
};
pub use server::{RemoteServer, next_global_seq};
