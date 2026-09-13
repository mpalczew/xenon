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

/// Read Xenon's own physical footprint, the value Activity Monitor calls Memory.
pub fn process_memory() -> ProcessMemory {
    #[cfg(target_os = "macos")]
    {
        return macos::process_memory();
    }
    #[allow(unreachable_code)]
    ProcessMemory::default()
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

#[cfg(target_os = "macos")]
mod macos {
    use super::ProcessMemory;

    type KernReturn = i32;
    type MachPort = u32;
    type Natural = u32;

    const TASK_VM_INFO: i32 = 22;
    const KERN_SUCCESS: KernReturn = 0;

    // task_vm_info's fields are laid out as 64-bit values except for the two
    // 32-bit values following virtual_size. These offsets match the macOS SDK.
    #[repr(C)]
    struct TaskVmInfo {
        fields: [u64; 40],
    }

    unsafe extern "C" {
        fn mach_task_self() -> MachPort;
        fn task_info(
            task: MachPort,
            flavor: i32,
            info: *mut u32,
            count: *mut Natural,
        ) -> KernReturn;
    }

    pub(super) fn process_memory() -> ProcessMemory {
        let mut info = TaskVmInfo { fields: [0; 40] };
        let mut count = (std::mem::size_of::<TaskVmInfo>() / std::mem::size_of::<u32>()) as Natural;
        let result = unsafe {
            task_info(
                mach_task_self(),
                TASK_VM_INFO,
                info.fields.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if result != KERN_SUCCESS {
            return ProcessMemory::default();
        }
        let footprint = info.fields[18];
        ProcessMemory {
            resident_bytes: info.fields[2],
            compressed_bytes: info.fields[15],
            footprint_bytes: footprint,
            // There is no separate phys-footprint peak field. The ledger peak
            // follows min_address and max_address.
            peak_footprint_bytes: footprint.max(info.fields[21]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_write_is_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        write_snapshot(&path, &ProcessMemory::default()).unwrap();
        let parsed: ProcessMemory = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(parsed, ProcessMemory::default());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn process_memory_has_sane_footprint() {
        let memory = process_memory();
        assert!(memory.footprint_bytes > 0);
        assert!(memory.peak_footprint_bytes >= memory.footprint_bytes);
        assert!(memory.compressed_bytes < memory.footprint_bytes.saturating_mul(100));
    }
}
