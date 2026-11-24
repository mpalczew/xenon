use std::time::Instant;

use xenon_store::RemoteDevice;

use crate::remote::auth::{ct_eq, hash_token, new_device_token};
use crate::remote::devices::DeviceBook;
use crate::remote::host::PairError;
use crate::remote::labels::{clean_label, unix_now};
use crate::remote::pairing::PairingCode;
use crate::remote::protocol::PairResponse;

const TOUCH_PERSIST_SECS: u64 = 60 * 60;

impl DeviceBook {
    pub fn pairing_code(&mut self, now: Instant) -> &PairingCode {
        if !self.pairing.as_ref().is_some_and(|c| c.is_live(now)) {
            self.pairing = Some(PairingCode::issue(now));
        }
        self.pairing.as_ref().expect("just issued")
    }

    pub(crate) fn pair(&mut self, code: &str, label: &str, now: Instant) -> Option<String> {
        if !self.pairing.as_ref().is_some_and(|c| c.matches(code, now)) {
            return None;
        }
        self.pairing = None;
        let id = new_device_token()[..32].to_string();
        let token = new_device_token();
        let stamp = unix_now();
        self.devices.devices.push(RemoteDevice {
            id,
            label: clean_label(label),
            token_sha256: hash_token(&token),
            created_at: stamp,
            last_seen_at: stamp,
        });
        Some(token)
    }

    pub(crate) fn authenticate(&self, token: &str) -> Option<String> {
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

    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.devices.len();
        self.devices.devices.retain(|d| d.id != id);
        before != self.devices.devices.len()
    }

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

    pub fn pair_response(
        &mut self,
        code: &str,
        label: &str,
        host_name: &str,
    ) -> Result<PairResponse, PairError> {
        self.reload();
        let token = self
            .pair(code, label, Instant::now())
            .ok_or(PairError::Rejected)?;
        self.save().map_err(|e| PairError::Storage(e.to_string()))?;
        Ok(PairResponse {
            token,
            host_name: host_name.to_string(),
        })
    }
}
