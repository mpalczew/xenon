//! Global managed Agent Skill. Not per-slot: lives under `$HOME`, not data_dir.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const BUNDLED: &str = include_str!("skill.md");
const SKILL_VERSION: u32 = 1;
const STATE_NAME: &str = ".xenon-skill.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillStatus {
    Missing,
    Installed { version: u32 },
    UpdateAvailable { installed: u32 },
    UserEdited,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct SkillState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    declined_version: Option<u32>,
}

/// User home, or `XENON_SKILL_HOME` (tests). Never the slot data dir.
pub fn skill_home() -> PathBuf {
    if let Some(dir) = std::env::var_os("XENON_SKILL_HOME") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn skill_status() -> SkillStatus {
    status_in(&skill_home())
}

pub fn install_skill() -> io::Result<()> {
    write_managed(&skill_home())?;
    save_state(&skill_home(), &SkillState::default())
}

pub fn remove_skill() -> io::Result<()> {
    let home = skill_home();
    for dir in skill_dirs(&home) {
        let path = dir.join("SKILL.md");
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let _ = fs::remove_dir(&dir);
    }
    Ok(())
}

pub fn decline_skill() -> io::Result<()> {
    save_state(
        &skill_home(),
        &SkillState {
            declined_version: Some(SKILL_VERSION),
        },
    )
}

pub fn should_prompt_skill() -> bool {
    if !matches!(skill_status(), SkillStatus::Missing) {
        return false;
    }
    !matches!(
        load_state(&skill_home()).declined_version,
        Some(v) if v >= SKILL_VERSION
    )
}

/// Rewrite managed copies that are older than this binary. Leave user edits.
pub fn refresh_on_launch() -> io::Result<()> {
    match skill_status() {
        SkillStatus::UpdateAvailable { .. } => write_managed(&skill_home()),
        SkillStatus::Missing | SkillStatus::Installed { .. } | SkillStatus::UserEdited => Ok(()),
    }
}

fn skill_dirs(home: &Path) -> [PathBuf; 3] {
    [
        home.join(".agents/skills/xenon"),
        home.join(".claude/skills/xenon"),
        home.join(".grok/skills/xenon"),
    ]
}

fn status_in(home: &Path) -> SkillStatus {
    let path = skill_dirs(home)[0].join("SKILL.md");
    if !path.exists() {
        return SkillStatus::Missing;
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return SkillStatus::UserEdited;
    };
    if !text.contains("xenon_managed: true") {
        return SkillStatus::UserEdited;
    }
    let mut version = 0;
    for line in text.lines().take(20) {
        if let Some(rest) = line.trim().strip_prefix("xenon_skill_version:") {
            version = rest.trim().parse().unwrap_or(0);
            break;
        }
    }
    if version < SKILL_VERSION {
        SkillStatus::UpdateAvailable { installed: version }
    } else {
        SkillStatus::Installed { version }
    }
}

fn write_managed(home: &Path) -> io::Result<()> {
    for dir in skill_dirs(home) {
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("SKILL.md"), BUNDLED)?;
    }
    Ok(())
}

fn load_state(home: &Path) -> SkillState {
    let Ok(text) = fs::read_to_string(home.join(STATE_NAME)) else {
        return SkillState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_state(home: &Path, state: &SkillState) -> io::Result<()> {
    let path = home.join(STATE_NAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_home<R>(body: impl FnOnce() -> R) -> R {
        let _guard = crate::DATA_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let prev = std::env::var_os("XENON_SKILL_HOME");
        unsafe {
            std::env::set_var("XENON_SKILL_HOME", dir.path());
        }
        let result = body();
        unsafe {
            match prev {
                Some(v) => std::env::set_var("XENON_SKILL_HOME", v),
                None => std::env::remove_var("XENON_SKILL_HOME"),
            }
        }
        result
    }

    fn read_canonical() -> String {
        fs::read_to_string(skill_dirs(&skill_home())[0].join("SKILL.md")).unwrap()
    }

    #[test]
    fn missing_installs_all_homes() {
        with_home(|| {
            assert_eq!(skill_status(), SkillStatus::Missing);
            assert!(should_prompt_skill());
            install_skill().unwrap();
            assert_eq!(
                skill_status(),
                SkillStatus::Installed {
                    version: SKILL_VERSION
                }
            );
            assert!(!should_prompt_skill());
            for dir in skill_dirs(&skill_home()) {
                let text = fs::read_to_string(dir.join("SKILL.md")).unwrap();
                assert!(text.contains("xenon_managed: true"));
                assert!(text.contains("xenon open"));
            }
        });
    }

    #[test]
    fn older_managed_rewrites() {
        with_home(|| {
            install_skill().unwrap();
            let stale = BUNDLED.replace("xenon_skill_version: 1", "xenon_skill_version: 0");
            fs::write(skill_dirs(&skill_home())[0].join("SKILL.md"), stale).unwrap();
            assert!(matches!(
                skill_status(),
                SkillStatus::UpdateAvailable { installed: 0 }
            ));
            refresh_on_launch().unwrap();
            assert!(read_canonical().contains("xenon_skill_version: 1"));
            assert_eq!(
                skill_status(),
                SkillStatus::Installed {
                    version: SKILL_VERSION
                }
            );
        });
    }

    #[test]
    fn same_version_is_noop() {
        with_home(|| {
            install_skill().unwrap();
            let before = read_canonical();
            refresh_on_launch().unwrap();
            assert_eq!(read_canonical(), before);
        });
    }

    #[test]
    fn user_edit_is_left_alone() {
        with_home(|| {
            let dir = skill_dirs(&skill_home())[0].clone();
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("SKILL.md"), "# my skill\n").unwrap();
            assert_eq!(skill_status(), SkillStatus::UserEdited);
            refresh_on_launch().unwrap();
            assert_eq!(
                fs::read_to_string(dir.join("SKILL.md")).unwrap(),
                "# my skill\n"
            );
            install_skill().unwrap();
            assert!(read_canonical().contains("xenon_managed: true"));
        });
    }

    #[test]
    fn decline_blocks_until_version_would_bump() {
        with_home(|| {
            assert!(should_prompt_skill());
            decline_skill().unwrap();
            assert!(!should_prompt_skill());
            assert_eq!(skill_status(), SkillStatus::Missing);
        });
    }

    #[test]
    fn writes_stay_in_temp_home() {
        with_home(|| {
            let home = skill_home();
            install_skill().unwrap();
            for dir in skill_dirs(&home) {
                assert!(dir.starts_with(&home));
            }
            remove_skill().unwrap();
            assert_eq!(skill_status(), SkillStatus::Missing);
        });
    }
}
