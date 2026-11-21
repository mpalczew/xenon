//! The phone server inside the service, as the window sees it.

use xenon_remote::RemoteStatus;

use crate::Client;
use crate::phone_api::{Pairing, PhoneCall, PhoneReply};
use crate::wire::{Message, Request};

/// The phone server inside the service: pairing code, status rows and settings.
pub struct PhoneLink<'a>(pub(crate) &'a Client);

impl PhoneLink<'_> {
    fn ask(&self, call: &PhoneCall) -> Result<PhoneReply, String> {
        let json = serde_json::to_string(call).map_err(|e| e.to_string())?;
        match self.0.call(Request::Phone { json }) {
            Ok(Message::Reply(_, json)) => serde_json::from_str(&json).map_err(|e| e.to_string()),
            Ok(Message::Err(_, e)) => Err(e),
            Ok(_) => Err("unexpected reply".into()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn status_of(&self, call: &PhoneCall) -> Result<RemoteStatus, String> {
        match self.ask(call)? {
            PhoneReply::Status { status } => Ok(status),
            PhoneReply::Pairing { .. } => Err("unexpected reply".into()),
        }
    }

    pub fn status(&self) -> Result<RemoteStatus, String> {
        self.status_of(&PhoneCall::Status)
    }

    /// Turn the phone server on or off, and remember the choice.
    pub fn set_enabled(&self, on: bool) -> Result<RemoteStatus, String> {
        self.status_of(&PhoneCall::SetEnabled { on })
    }

    /// Sign out one device, or all of them.
    pub fn revoke(&self, device: Option<&str>) -> Result<RemoteStatus, String> {
        self.status_of(&PhoneCall::Revoke {
            device: device.map(str::to_owned),
        })
    }

    /// The current pairing code, turning the phone server on first if it is off.
    pub fn pairing(&self) -> Result<Pairing, String> {
        match self.ask(&PhoneCall::Pairing)? {
            PhoneReply::Pairing { pairing } => pairing,
            PhoneReply::Status { .. } => Err("unexpected reply".into()),
        }
    }
}
