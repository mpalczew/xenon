use std::path::{Path, PathBuf};

/// Data dir (and optional A/B slot) the stub should export before exec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotLaunch {
    pub data_dir: PathBuf,
    pub slot: Option<String>,
}

/// Resolve launch env from existing data-dir env and `$HOME` slot files.
///
/// A set `XENON_DATA_DIR` or `XERO_DATA_DIR` is returned unchanged (no slot).
/// Otherwise `~/.xenon-active-slot` of `a`/`b` plus a matching `~/.xenon-{slot}`
/// directory selects that slot. Missing marker or dir → ship `~/.xenon`.
pub fn resolve_slot_launch(
    home: &Path,
    xenon_data_dir: Option<&Path>,
    xero_data_dir: Option<&Path>,
) -> SlotLaunch {
    if let Some(dir) = xenon_data_dir.or(xero_data_dir) {
        return SlotLaunch {
            data_dir: dir.to_path_buf(),
            slot: None,
        };
    }
    if let Some(name) = active_slot(home) {
        let data_dir = home.join(format!(".xenon-{name}"));
        if data_dir.is_dir() {
            return SlotLaunch {
                slot: Some(name.to_uppercase()),
                data_dir,
            };
        }
    }
    SlotLaunch {
        data_dir: home.join(".xenon"),
        slot: None,
    }
}

/// Export resolved slot env. No-op when a data-dir env var is already set.
pub fn apply_slot_env() {
    if std::env::var_os("XENON_DATA_DIR").is_some() || std::env::var_os("XERO_DATA_DIR").is_some() {
        return;
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let launch = resolve_slot_launch(&home, None, None);
    // SAFETY: single-threaded stub, before exec of the GUI.
    unsafe {
        std::env::set_var("XENON_DATA_DIR", &launch.data_dir);
        std::env::set_var("XERO_DATA_DIR", &launch.data_dir);
        if let Some(slot) = &launch.slot {
            std::env::set_var("XENON_SLOT", slot);
            std::env::set_var("XERO_SLOT", slot);
        }
    }
}

fn active_slot(home: &Path) -> Option<&'static str> {
    let raw = std::fs::read_to_string(home.join(".xenon-active-slot")).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "a" => Some("a"),
        "b" => Some("b"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SlotLaunch, resolve_slot_launch};
    use std::fs;
    use std::path::PathBuf;

    fn home() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    #[test]
    fn marker_and_slot_dir_selects_that_slot() {
        let (_keep, home) = home();
        fs::create_dir(home.join(".xenon-a")).unwrap();
        fs::write(home.join(".xenon-active-slot"), "a\n").unwrap();
        assert_eq!(
            resolve_slot_launch(&home, None, None),
            SlotLaunch {
                data_dir: home.join(".xenon-a"),
                slot: Some("A".into()),
            }
        );
    }

    #[test]
    fn missing_marker_uses_ship_dir() {
        let (_keep, home) = home();
        assert_eq!(
            resolve_slot_launch(&home, None, None),
            SlotLaunch {
                data_dir: home.join(".xenon"),
                slot: None,
            }
        );
    }

    #[test]
    fn existing_data_dir_env_is_unchanged() {
        let (_keep, home) = home();
        fs::create_dir(home.join(".xenon-b")).unwrap();
        fs::write(home.join(".xenon-active-slot"), "b").unwrap();
        let preset = home.join("preset");
        assert_eq!(
            resolve_slot_launch(&home, Some(&preset), None),
            SlotLaunch {
                data_dir: preset,
                slot: None,
            }
        );
    }

    #[test]
    fn marker_without_slot_dir_uses_ship_dir() {
        let (_keep, home) = home();
        fs::write(home.join(".xenon-active-slot"), "b").unwrap();
        assert_eq!(
            resolve_slot_launch(&home, None, None).data_dir,
            home.join(".xenon")
        );
    }
}
