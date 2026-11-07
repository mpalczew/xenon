//! Paired phones and the one-time pairing code. Tokens are only ever stored
//! hashed; the plain token goes to the phone once, in the pair response.
//! Both frontends keep one `DeviceBook` while the remote is on; the pairing
//! and sign-in steps are in `device_flow.rs`.

use xenon_store::{RemoteDevice, RemoteDevices};

use crate::pairing::PairingCode;

pub struct DeviceBook {
    pub(crate) devices: RemoteDevices,
    pub(crate) pairing: Option<PairingCode>,
}

impl DeviceBook {
    pub fn load() -> Self {
        let devices = xenon_store::load_remote_devices().unwrap_or_else(|e| {
            log::warn!("remote devices unreadable: {e}");
            RemoteDevices::default()
        });
        Self::from_devices(devices)
    }

    pub fn from_devices(devices: RemoteDevices) -> Self {
        Self {
            devices,
            pairing: None,
        }
    }

    /// Re-read the shared file (another Xenon may have changed it). Keeps the
    /// pairing code; a read failure keeps the current list.
    pub fn reload(&mut self) {
        match xenon_store::load_remote_devices() {
            Ok(devices) => self.devices = devices,
            Err(e) => log::warn!("remote devices reload: {e}"),
        }
    }

    pub fn devices(&self) -> &[RemoteDevice] {
        &self.devices.devices
    }

    pub fn label(&self, id: &str) -> Option<&str> {
        self.devices
            .devices
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.label.as_str())
    }

    /// Sign out every device and invalidate any shown code.
    pub fn revoke_all(&mut self) {
        self.devices.devices.clear();
        self.pairing = None;
    }

    pub fn save(&self) -> Result<(), xenon_store::StoreError> {
        xenon_store::save_remote_devices(&self.devices)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

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
        assert!(book.pair("000000", "x", now).is_none());
    }
}
