//! Paired phones and the one-time pairing code. Tokens are only ever stored
//! hashed; the plain token goes to the phone once, in the pair response.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use xenon_remote::{PairingCode, ct_eq, hash_token, new_device_token};
use xenon_store::{RemoteDevice, RemoteDevices};

/// `last_seen_at` is persisted at most this often per device.
const TOUCH_PERSIST_SECS: u64 = 60 * 60;

pub(crate) struct DeviceBook {
    devices: RemoteDevices,
    pairing: Option<PairingCode>,
}

/// A device just paired: its id and the only copy of its plain token.
pub(crate) struct NewDevice {
    pub id: String,
    pub token: String,
}

impl DeviceBook {
    pub(crate) fn load() -> Self {
        let devices = xenon_store::load_remote_devices().unwrap_or_else(|e| {
            log::warn!("remote devices unreadable: {e}");
            RemoteDevices::default()
        });
        Self::from_devices(devices)
    }

    pub(crate) fn from_devices(devices: RemoteDevices) -> Self {
        Self {
            devices,
            pairing: None,
        }
    }

    pub(crate) fn devices(&self) -> &[RemoteDevice] {
        &self.devices.devices
    }

    /// The live pairing code, issuing a fresh one when missing or expired.
    pub(crate) fn pairing_code(&mut self, now: Instant) -> &PairingCode {
        if !self.pairing.as_ref().is_some_and(|c| c.is_live(now)) {
            self.pairing = Some(PairingCode::issue(now));
        }
        self.pairing.as_ref().expect("just issued")
    }

    /// The sheet closed: stop accepting its code.
    pub(crate) fn forget_pairing(&mut self) {
        self.pairing = None;
    }

    #[cfg(feature = "visual-tests")]
    pub(crate) fn visual_set_pairing(&mut self, code: PairingCode) {
        self.pairing = Some(code);
    }

    /// Consume the pairing code and add a device. None = wrong/expired code.
    /// The code is single use; a fresh one replaces it for the next phone.
    pub(crate) fn pair(&mut self, code: &str, label: &str, now: Instant) -> Option<NewDevice> {
        if !self.pairing.as_ref().is_some_and(|c| c.matches(code, now)) {
            return None;
        }
        // Single use; the sheet issues a fresh one if it is still open.
        self.pairing = None;
        let id = uuid_simple();
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

    /// Record use. True when the change is worth persisting now.
    pub(crate) fn touch(&mut self, id: &str) -> bool {
        let now = unix_now();
        let Some(device) = self.devices.devices.iter_mut().find(|d| d.id == id) else {
            return false;
        };
        let stale = now.saturating_sub(device.last_seen_at) >= TOUCH_PERSIST_SECS;
        device.last_seen_at = now;
        stale
    }

    pub(crate) fn label(&self, id: &str) -> Option<&str> {
        self.devices
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.label.as_str())
    }

    pub(crate) fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.devices.len();
        self.devices.devices.retain(|d| d.id != id);
        before != self.devices.devices.len()
    }

    /// Sign out every device and invalidate any shown code.
    pub(crate) fn revoke_all(&mut self) {
        self.devices.devices.clear();
        self.pairing = None;
    }

    pub(crate) fn save(&self) -> Result<(), xenon_store::StoreError> {
        xenon_store::save_remote_devices(&self.devices)
    }
}

/// "iPhone · Home Screen" → "iPhone" for the terminal banner.
pub(crate) fn device_short_name(label: &str) -> &str {
    label.split(" · ").next().unwrap_or(label).trim()
}

fn clean_label(label: &str) -> String {
    let label: String = label.chars().filter(|c| !c.is_control()).take(40).collect();
    let label = label.trim();
    if label.is_empty() {
        "Phone".to_string()
    } else {
        label.to_string()
    }
}

fn uuid_simple() -> String {
    // Token generator shape without its length: a fresh id, not a secret.
    new_device_token()[..32].to_string()
}

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_adds_device_and_rotates_code() {
        let now = Instant::now();
        let mut book = DeviceBook::from_devices(RemoteDevices::default());
        let code = book.pairing_code(now).digits.clone();
        let device = book.pair(&code, "iPhone · Safari", now).expect("paired");
        assert_eq!(book.authenticate(&device.token), Some(device.id.clone()));
        assert!(book.authenticate("nope").is_none());
        // Single use: the same code no longer works.
        assert!(book.pair(&code, "Other", now).is_none());
        assert_ne!(book.pairing_code(now).digits, code);
    }

    #[test]
    fn stores_only_the_token_hash() {
        let now = Instant::now();
        let mut book = DeviceBook::from_devices(RemoteDevices::default());
        let code = book.pairing_code(now).secret.clone();
        let device = book.pair(&code, "", now).unwrap();
        let stored = &book.devices()[0];
        assert_ne!(stored.token_sha256, device.token);
        assert_eq!(stored.label, "Phone");
    }

    #[test]
    fn revoke_all_signs_everyone_out() {
        let now = Instant::now();
        let mut book = DeviceBook::from_devices(RemoteDevices::default());
        let code = book.pairing_code(now).digits.clone();
        let device = book.pair(&code, "iPhone", now).unwrap();
        book.revoke_all();
        assert!(book.authenticate(&device.token).is_none());
        // Old codes die with it: a fresh one is issued on next use.
        assert!(book.pair("000000", "x", now).is_none());
    }

    #[test]
    fn short_name_takes_the_device_part() {
        assert_eq!(device_short_name("iPhone · Home Screen"), "iPhone");
        assert_eq!(device_short_name("Pixel"), "Pixel");
    }
}
