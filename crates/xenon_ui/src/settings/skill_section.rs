//! Settings → Agents: install / update / reset / remove the managed skill.

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use theme::ActiveTheme;
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
    let check_bg = if checked {
        colors.element_selected
    } else {
        colors.elevated_surface_background
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
                    toggle_skill(cx);
                    window.refresh();
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().child("Agent skill"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(subtitle),
                        ),
                )
                .child(
                    div()
                        .w(px(18.))
                        .h(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(colors.border)
                        .bg(check_bg)
                        .text_xs()
                        .children(checked.then_some("x")),
                ),
        )
        .child(row_divider(cx))
        .child(
            div()
                .px_3()
                .py_2()
                .text_xs()
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
            row = row.child(action_btn("Install skill", true, cx, |_| {
                let _ = install_skill();
            }));
        }
        SkillStatus::Installed { .. } => {
            row = row.child(action_btn("Remove", false, cx, |_| {
                let _ = remove_skill();
            }));
        }
        SkillStatus::UpdateAvailable { .. } => {
            row = row
                .child(action_btn("Update to bundled", true, cx, |_| {
                    let _ = install_skill();
                }))
                .child(action_btn("Remove", false, cx, |_| {
                    let _ = remove_skill();
                }));
        }
        SkillStatus::UserEdited => {
            row = row
                .child(action_btn("Reset to bundled", true, cx, |_| {
                    let _ = install_skill();
                }))
                .child(action_btn("Remove", false, cx, |_| {
                    let _ = remove_skill();
                }));
        }
    }
    row
}

fn action_btn(
    label: &'static str,
    primary: bool,
    cx: &mut Context<SettingsView>,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let bg = if primary {
        colors.text_accent
    } else {
        colors.background
    };
    let fg = if primary {
        colors.background
    } else {
        colors.text
    };
    div()
        .id(label)
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(bg)
        .text_color(fg)
        .text_xs()
        .cursor_pointer()
        .on_click(cx.listener(move |_, _, window, cx| {
            on_click(cx);
            window.refresh();
            cx.notify();
        }))
        .child(label)
}

pub(super) fn toggle_skill_from_keys(cx: &mut App) {
    toggle_skill(cx);
}

fn toggle_skill(_cx: &mut App) {
    match skill_status() {
        SkillStatus::Missing => {
            let _ = install_skill();
        }
        _ => {
            let _ = remove_skill();
        }
    }
}
