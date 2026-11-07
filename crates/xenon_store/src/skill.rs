//! Global managed Agent Skill. Not per-slot: lives under `$HOME`, not data_dir.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const BUNDLED: &str = include_str!("skill.md");
const SKILL_VERSION: u32 = 9;
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
pub(crate) fn skill_home() -> PathBuf {
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

fn save_state(home: &Path, state: &SkillState) -> io::Result<()> {
    let path = home.join(STATE_NAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state).unwrap())
}
