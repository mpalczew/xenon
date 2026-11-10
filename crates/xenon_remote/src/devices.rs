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

    pub fn revoke_all(&mut self) {
        self.devices.devices.clear();
        self.pairing = None;
    }

    pub fn save(&self) -> Result<(), xenon_store::StoreError> {
        xenon_store::save_remote_devices(&self.devices)
    }
}
