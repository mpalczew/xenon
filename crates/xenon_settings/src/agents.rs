use xenon_store::SkillStatus;

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

pub fn skill_refresh_label(status: &SkillStatus) -> Option<&'static str> {
    match status {
        SkillStatus::UpdateAvailable { .. } => Some("Update to bundled"),
        SkillStatus::UserEdited => Some("Reset to bundled"),
        SkillStatus::Missing | SkillStatus::Installed { .. } => None,
    }
}

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
