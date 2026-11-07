//! Text for the Agents and Language Servers settings: what the managed skill's
//! status says, and how a typed server command is stored.

use xenon_store::{LspSettings, SkillStatus};

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

/// Which language server a command belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LspServer {
    Rust,
    TypeScript,
}

/// Store an edited server command; empty text means find it on PATH.
pub fn set_lsp_command(lsp: &mut LspSettings, server: LspServer, text: &str) {
    let command = Some(text.trim().to_string()).filter(|c| !c.is_empty());
    match server {
        LspServer::Rust => lsp.rust.command = command,
        LspServer::TypeScript => lsp.typescript.command = command,
    }
}
