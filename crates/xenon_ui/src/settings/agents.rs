//! Settings › Agents: install, update, reset, or remove the managed skill.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, Window};
use xenon_design_system::{Toast, show_toast_in};
use xenon_settings::{skill_homes, skill_refresh_label, skill_subtitle};
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
        .detail(skill_subtitle(&pinned(&status)))
        .keywords("skill claude codex grok install"),
        SettingRow::new("agent-skill-homes", "Installed in", Control::None)
            .detail(skill_homes(&pinned(&status)))
            .tone(Tone::Mono)
            .keywords("skill path"),
    ];
    if let Some(label) = skill_refresh_label(&status) {
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

/// Scenes pin the shown version; the shared text reads it from the status.
fn pinned(status: &SkillStatus) -> SkillStatus {
    match *status {
        SkillStatus::Installed { version } => SkillStatus::Installed {
            version: shown(version),
        },
        SkillStatus::UpdateAvailable { installed } => SkillStatus::UpdateAvailable {
            installed: shown(installed),
        },
        other => other,
    }
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
