//! What the window can ask of the phone server over `phone.sock`: one JSON object per
//! line, one reply line per call. Tolerant serde: unknown fields are ignored, so the
//! window and the phone server may differ in build until the window restarts.

use serde::{Deserialize, Serialize};
use xenon_remote::RemoteStatus;

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum PhoneCall {
    /// Status of the phone server. The server first re-reads `services.json`, which the
    /// window has just written when it changed a setting.
    Status,
    Pairing,
    Revoke {
        device: Option<String>,
    },
    /// Whether the window's theme is dark: phones follow it by default.
    Dark {
        dark: bool,
    },
    /// Keep this connection and push `Changed` whenever the status changes.
    Watch,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PhoneReply {
    Status { status: RemoteStatus },
    Pairing { pairing: Result<Pairing, String> },
    Changed,
    Done,
    Error { error: String },
}

/// A live pairing code. `url` is `None` when the phone could not reach this computer
/// (Tailscale is not running and no address override is set).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pairing {
    pub digits: String,
    pub url: Option<String>,
    pub expires_in_secs: u64,
}
