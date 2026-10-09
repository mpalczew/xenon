//! Settings › Agents: install, update, reset, or remove the managed skill.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, Window};
use xenon_design_system::{Toast, show_toast_in};
use xenon_store::{SkillStatus, install_skill, remove_skill, skill_status};

use super::row::{ActionKind, Control, Group, RowAction, SettingRow, Tone};

thread_local! {
    /// Screenshot scenes pin the shown version so baselines survive skill bumps.
    static PINNED_VERSION: Cell<Option<u32>> = const { Cell::new(None) };
}

#[cfg(feature = "visual-tests")]
pub(crate) fn pin_version_for_visuals(version: u32) {
    PINNED_VERSION.set(Some(version));
}

fn shown(version: u32) -> u32 {
    PINNED_VERSION.get().unwrap_or(version)
}

pub(super) fn groups() -> Vec<Group> {
    let status = skill_status();
    let installed = !matches!(status, SkillStatus::Missing);
    let mut rows = vec![
        SettingRow::new(
            "agent-skill",
            "Agent skill",
            Control::Switch {
                on: installed,
                toggle: Rc::new(toggle_skill),
            },
        )
        .detail(subtitle(&status))
        .keywords("skill claude codex grok install"),
        SettingRow::new("agent-skill-homes", "Installed in", Control::None)
            .detail(homes_label(&status))
            .tone(Tone::Mono)
            .keywords("skill path"),
    ];
    if let Some(label) = refresh_label(&status) {
        rows.push(
            SettingRow::new(
                "agent-skill-refresh",
                "Bundled skill",
                Control::Action(RowAction {
                    label: label.into(),
                    kind: ActionKind::Primary,
                    run: Rc::new(|window, cx| install(window, cx)),
                }),
            )
            .detail("Replace your copy with the one shipped in this build")
            .keywords("skill update reset"),
        );
    }
    vec![Group::untitled(rows)]
}

fn subtitle(status: &SkillStatus) -> String {
    match status {
        SkillStatus::Missing => "Off — Xenon asks when an agent starts".into(),
        SkillStatus::Installed { version } => {
            format!("Installed v{} · same skill for every slot", shown(*version))
        }
        SkillStatus::UpdateAvailable { installed } => {
            format!(
                "Installed v{} · a newer one ships with this build",
                shown(*installed)
            )
        }
        SkillStatus::UserEdited => "You edited the skill · Xenon will not overwrite it".into(),
    }
}

fn refresh_label(status: &SkillStatus) -> Option<&'static str> {
    match status {
        SkillStatus::UpdateAvailable { .. } => Some("Update to bundled"),
        SkillStatus::UserEdited => Some("Reset to bundled"),
        SkillStatus::Missing | SkillStatus::Installed { .. } => None,
    }
}

fn homes_label(status: &SkillStatus) -> String {
    let mark = match status {
        SkillStatus::Missing => "not installed".to_string(),
        SkillStatus::Installed { version } => format!("v{}", shown(*version)),
        SkillStatus::UpdateAvailable { installed } => format!("v{}", shown(*installed)),
        SkillStatus::UserEdited => "edited".to_string(),
    };
    ["~/.agents", "~/.claude", "~/.grok"]
        .map(|home| format!("{:<22} {mark}", format!("{home}/skills/xenon")))
        .join("\n")
}

fn toggle_skill(window: &mut Window, cx: &mut App) {
    match skill_status() {
        SkillStatus::Missing => install(window, cx),
        _ => remove(window, cx),
    }
}

fn install(window: &Window, cx: &mut App) {
    let toast = match install_skill() {
        Ok(()) => Toast::success("🧩", "Xenon skill installed"),
        Err(error) => Toast::error("🙈", "Couldn’t install the skill").detail(error.to_string()),
    };
    show_toast_in(window, toast, cx);
}

fn remove(window: &Window, cx: &mut App) {
    let toast = match remove_skill() {
        Ok(()) => Toast::info("🧹", "Xenon skill removed"),
        Err(error) => Toast::error("🙈", "Couldn’t remove the skill").detail(error.to_string()),
    };
    show_toast_in(window, toast, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homes_list_every_agent_home() {
        let label = homes_label(&SkillStatus::Installed { version: 3 });
        assert_eq!(label.lines().count(), 3);
        assert!(label.contains("~/.claude/skills/xenon"));
        assert!(label.ends_with("v3"));
    }
}
