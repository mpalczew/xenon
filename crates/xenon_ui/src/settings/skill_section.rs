//! Settings → Agents: install / update / reset / remove the managed skill.

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div,
};
use theme::ActiveTheme;
use xenon_design_system::{Toast, TypeRole, Typography, show_toast_in};
use xenon_store::{SkillStatus, install_skill, remove_skill, skill_status};

use super::SettingsView;
use super::sections::{group_card, row_divider};

pub(super) fn skill_section(focused: bool, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let status = skill_status();
    let (subtitle, checked) = match status {
        SkillStatus::Missing => ("Off — prompt when an agent starts".to_string(), false),
        SkillStatus::Installed { version } => (
            format!("Installed v{version} · same skill for every slot"),
            true,
        ),
        SkillStatus::UpdateAvailable { installed } => (
            format!("Installed v{installed} · bundled newer — will refresh on next launch"),
            true,
        ),
        SkillStatus::UserEdited => (
            "You edited the skill · Xenon will not overwrite it".to_string(),
            true,
        ),
    };
    let homes = homes_label(&status);
    let colors = cx.theme().colors().clone();
    let row_bg = if focused {
        colors.element_hover
    } else {
        gpui::transparent_black()
    };
    let body = div()
        .flex()
        .flex_col()
        .child(
            div()
                .id("agent-skill-toggle")
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .bg(row_bg)
                .cursor_pointer()
                .on_click(cx.listener(|_, _, window, cx| {
                    toggle_skill(window, cx);
                    window.refresh();
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().type_role(TypeRole::Body, cx).child("Agent skill"))
                        .child(
                            div()
                                .type_role(TypeRole::ControlLabel, cx)
                                .text_color(colors.text_muted)
                                .child(subtitle),
                        ),
                )
                .child(xenon_design_system::checkbox(
                    "agent-skill-checkbox",
                    xenon_design_system::CheckboxState {
                        checked,
                        disabled: false,
                    },
                    "Agent skill",
                    cx,
                    cx.listener(|_, _, window, cx| {
                        cx.stop_propagation();
                        toggle_skill(window, cx);
                        window.refresh();
                        cx.notify();
                    }),
                )),
        )
        .child(row_divider(cx))
        .child(
            div()
                .px_3()
                .py_2()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .font_family("ui-monospace")
                .child(homes),
        )
        .child(row_divider(cx))
        .child(action_row(status, cx));
    group_card("Agents", body, cx)
}

fn homes_label(status: &SkillStatus) -> String {
    let mark = match status {
        SkillStatus::Missing => "not installed",
        SkillStatus::Installed { version } => {
            return format!(
                "~/.agents/skills/xenon   v{version}  managed\n~/.claude/skills/xenon   v{version}  copy\n~/.grok/skills/xenon     v{version}  copy"
            );
        }
        SkillStatus::UpdateAvailable { installed } => {
            return format!(
                "~/.agents/skills/xenon   v{installed}  managed\n~/.claude/skills/xenon   v{installed}  copy\n~/.grok/skills/xenon     v{installed}  copy"
            );
        }
        SkillStatus::UserEdited => "edited, not managed",
    };
    format!(
        "~/.agents/skills/xenon   {mark}\n~/.claude/skills/xenon   {mark}\n~/.grok/skills/xenon     {mark}"
    )
}

fn action_row(status: SkillStatus, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let mut row = div().flex().justify_end().gap_2().px_3().py_2();
    match status {
        SkillStatus::Missing => {
            row = row.child(action_btn("Install skill", true, cx, install));
        }
        SkillStatus::Installed { .. } => {
            row = row.child(action_btn("Remove", false, cx, remove));
        }
        SkillStatus::UpdateAvailable { .. } => {
            row = row
                .child(action_btn("Update to bundled", true, cx, install))
                .child(action_btn("Remove", false, cx, remove));
        }
        SkillStatus::UserEdited => {
            row = row
                .child(action_btn("Reset to bundled", true, cx, install))
                .child(action_btn("Remove", false, cx, remove));
        }
    }
    row
}

fn action_btn(
    label: &'static str,
    primary: bool,
    cx: &mut Context<SettingsView>,
    on_click: fn(&Window, &mut App),
) -> impl IntoElement {
    xenon_design_system::action_button(
        label,
        if primary {
            xenon_design_system::ActionButton::primary(label)
        } else {
            xenon_design_system::ActionButton::secondary(label)
        },
        cx,
        cx.listener(move |_, _, window, cx| {
            on_click(window, cx);
            window.refresh();
            cx.notify();
        }),
    )
}

pub(super) fn toggle_skill(window: &Window, cx: &mut App) {
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
