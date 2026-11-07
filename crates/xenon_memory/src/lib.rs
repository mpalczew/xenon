//! Process memory diagnostics shared by Xenon's UI and local AI tooling.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessMemory {
    pub footprint_bytes: u64,
    pub peak_footprint_bytes: u64,
    pub resident_bytes: u64,
    pub compressed_bytes: u64,
}

/// Read Xenon's own memory use from `/proc`.
pub fn process_memory() -> ProcessMemory {
    linux::process_memory()
}

/// Write a complete, redacted snapshot atomically for local tools to inspect.
pub fn write_snapshot<T: Serialize>(path: &Path, snapshot: &T) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(temp, path)
}

/// There is no compressed memory or physical-footprint ledger: footprint is
/// resident pages, the peak is `VmHWM`.
mod linux {
    use super::ProcessMemory;

    pub(super) fn process_memory() -> ProcessMemory {
        let page = 4096;
        let resident = std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map_or(0, |pages| pages * page);
        let peak = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
                line.split_whitespace().nth(1)?.parse::<u64>().ok()
            })
            .map_or(resident, |kib| kib * 1024);
        ProcessMemory {
            footprint_bytes: resident,
            peak_footprint_bytes: peak.max(resident),
            resident_bytes: resident,
            compressed_bytes: 0,
        }
    }
}
