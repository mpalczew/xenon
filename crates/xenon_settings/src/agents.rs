//! Text for the Agents settings: what the managed skill's
//! status says.

use xenon_store::SkillStatus;

/// The line under the Agent skill switch.
pub fn skill_subtitle(status: &SkillStatus) -> String {
    match status {
        SkillStatus::Missing => "Off — Xenon asks when an agent starts".into(),
        SkillStatus::Installed { version } => {
            format!("Installed v{version} · same skill for every slot")
        }
        SkillStatus::UpdateAvailable { installed } => {
            format!("Installed v{installed} · a newer one ships with this build")
        }
        SkillStatus::UserEdited => "You edited the skill · Xenon will not overwrite it".into(),
    }
}

/// The button that replaces the installed copy, when one makes sense.
pub fn skill_refresh_label(status: &SkillStatus) -> Option<&'static str> {
    match status {
        SkillStatus::UpdateAvailable { .. } => Some("Update to bundled"),
        SkillStatus::UserEdited => Some("Reset to bundled"),
        SkillStatus::Missing | SkillStatus::Installed { .. } => None,
    }
}

/// One line per agent home with the installed state, for a mono block.
pub fn skill_homes(status: &SkillStatus) -> String {
    let mark = match status {
        SkillStatus::Missing => "not installed".to_string(),
        SkillStatus::Installed { version } => format!("v{version}"),
        SkillStatus::UpdateAvailable { installed } => format!("v{installed}"),
        SkillStatus::UserEdited => "edited".to_string(),
    };
    ["~/.agents", "~/.claude", "~/.grok"]
        .map(|home| format!("{:<22} {mark}", format!("{home}/skills/xenon")))
        .join("\n")
}
