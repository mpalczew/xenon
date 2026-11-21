//! What the window can ask of the phone server inside the service (JSON on the wire).

use serde::{Deserialize, Serialize};
use xenon_remote::RemoteStatus;

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum PhoneCall {
    Status,
    SetEnabled { on: bool },
    Pairing,
    Revoke { device: Option<String> },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PhoneReply {
    Status { status: RemoteStatus },
    Pairing { pairing: Result<Pairing, String> },
}

/// A live pairing code. `url` is `None` when the phone could not reach this computer
/// (Tailscale is not running and no address override is set).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pairing {
    pub digits: String,
    pub url: Option<String>,
    pub expires_in_secs: u64,
}
