//! The steps of a device's life on a `DeviceBook`: show a pairing code, pair,
//! sign in with a token, revoke.

use std::time::Instant;

use xenon_store::RemoteDevice;

use crate::auth::{ct_eq, hash_token, new_device_token};
use crate::devices::DeviceBook;
use crate::host::PairError;
use crate::labels::{clean_label, unix_now};
use crate::pairing::PairingCode;
use crate::protocol::PairResponse;

/// `last_seen_at` is persisted at most this often per device.
const TOUCH_PERSIST_SECS: u64 = 60 * 60;

/// A device just paired: its id and the only copy of its plain token.
pub(crate) struct NewDevice {
    pub id: String,
    pub token: String,
}

impl DeviceBook {
    /// The live pairing code, issuing a fresh one when missing or expired.
    pub fn pairing_code(&mut self, now: Instant) -> &PairingCode {
        if !self.pairing.as_ref().is_some_and(|c| c.is_live(now)) {
            self.pairing = Some(PairingCode::issue(now));
        }
        self.pairing.as_ref().expect("just issued")
    }

    /// The sheet closed: stop accepting its code.
    pub fn forget_pairing(&mut self) {
        self.pairing = None;
    }

    /// Show a fixed code (screenshots and tests).
    pub fn set_pairing(&mut self, code: PairingCode) {
        self.pairing = Some(code);
    }

    /// Consume the pairing code and add a device. None = wrong/expired code.
    /// The code is single use; a fresh one replaces it for the next phone.
    pub(crate) fn pair(&mut self, code: &str, label: &str, now: Instant) -> Option<NewDevice> {
        if !self.pairing.as_ref().is_some_and(|c| c.matches(code, now)) {
            return None;
        }
        self.pairing = None;
        let id = new_device_token()[..32].to_string();
        let token = new_device_token();
        let stamp = unix_now();
        self.devices.devices.push(RemoteDevice {
            id: id.clone(),
            label: clean_label(label),
            token_sha256: hash_token(&token),
            created_at: stamp,
            last_seen_at: stamp,
        });
        Some(NewDevice { id, token })
    }

    /// Device id for a presented token.
    pub fn authenticate(&self, token: &str) -> Option<String> {
        if token.is_empty() {
            return None;
        }
        let hash = hash_token(token);
        self.devices
            .devices
            .iter()
            .find(|d| ct_eq(&d.token_sha256, &hash))
            .map(|d| d.id.clone())
    }

    /// Forget one device.
    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.devices.len();
        self.devices.devices.retain(|d| d.id != id);
        before != self.devices.devices.len()
    }

    /// `HostRequest::Authenticate`: re-read the file, check the token, and
    /// record the use (persisted at most hourly).
    pub fn authenticate_touch(&mut self, token: &str) -> Option<String> {
        self.reload();
        let id = self.authenticate(token)?;
        let now = unix_now();
        if let Some(device) = self.devices.devices.iter_mut().find(|d| d.id == id) {
            let stale = now.saturating_sub(device.last_seen_at) >= TOUCH_PERSIST_SECS;
            device.last_seen_at = now;
            if stale && let Err(e) = self.save() {
                log::warn!("remote devices save: {e}");
            }
        }
        Some(id)
    }

    /// `HostRequest::Pair`: spend the code, save, and build the reply.
    pub fn pair_response(
        &mut self,
        code: &str,
        label: &str,
        host_name: &str,
    ) -> Result<PairResponse, PairError> {
        self.reload();
        let device = self
            .pair(code, label, Instant::now())
            .ok_or(PairError::Rejected)?;
        self.save().map_err(|e| PairError::Storage(e.to_string()))?;
        Ok(PairResponse {
            device_id: device.id,
            token: device.token,
            host_name: host_name.to_string(),
        })
    }
}
