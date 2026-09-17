//! In-app "Install the Xenon skill?" dialog. Keyboard: Return / Escape / arrows.

use super::*;
use xenon_store::{decline_skill, install_skill, should_prompt_skill};

const BTN_NEVER: usize = 0;
const BTN_LATER: usize = 1;
const BTN_INSTALL: usize = 2;

#[derive(Clone, Copy)]
pub(crate) struct SkillPrompt {
    pub button: usize,
}

impl XenonApp {
    pub(crate) fn maybe_offer_skill(&mut self, title: &str, cx: &mut Context<Self>) {
        if self.skill_prompt.is_some() || self.skill_skipped_session {
            return;
        }
        if !xenon_terminal::agent_title(title) {
            return;
        }
        if !should_prompt_skill() {
            return;
        }
        self.skill_prompt = Some(SkillPrompt {
            button: BTN_INSTALL,
        });
        cx.notify();
    }

    #[cfg(feature = "visual-tests")]
    pub(crate) fn show_skill_prompt(&mut self, cx: &mut Context<Self>) {
        self.skill_prompt = Some(SkillPrompt {
            button: BTN_INSTALL,
        });
        cx.notify();
    }

    pub(crate) fn on_skill_prompt_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.skill_prompt.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_skill_later(cx);
                true
            }
            "enter" | "return" => {
                self.confirm_skill_prompt(cx);
                true
            }
            "left" => {
                if let Some(prompt) = &mut self.skill_prompt {
                    prompt.button = prompt.button.saturating_sub(1);
                    cx.notify();
                }
                true
            }
            "right" => {
                if let Some(prompt) = &mut self.skill_prompt {
                    prompt.button = (prompt.button + 1).min(BTN_INSTALL);
                    cx.notify();
                }
                true
            }
            _ => false,
        }
    }

    fn confirm_skill_prompt(&mut self, cx: &mut Context<Self>) {
        let button = self.skill_prompt.map(|p| p.button).unwrap_or(BTN_INSTALL);
        match button {
            BTN_NEVER => {
                let _ = decline_skill();
                self.skill_prompt = None;
                self.skill_skipped_session = true;
            }
            BTN_LATER => self.dismiss_skill_later(cx),
            _ => {
                if let Err(error) = install_skill() {
                    log::error!("skill install: {error}");
                }
                self.skill_prompt = None;
            }
        }
        cx.notify();
    }

    fn dismiss_skill_later(&mut self, cx: &mut Context<Self>) {
        self.skill_prompt = None;
        self.skill_skipped_session = true;
        cx.notify();
    }

    fn pick_skill_button(&mut self, button: usize, cx: &mut Context<Self>) {
        self.skill_prompt = Some(SkillPrompt { button });
        self.confirm_skill_prompt(cx);
    }

    pub(crate) fn render_skill_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let scrim = colors.background.opacity(0.55);
        let dialog = div()
            .id("skill-prompt-dialog")
            .occlude()
            .w(px(420.))
            .p_4()
            .rounded_lg()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child("Install the Xenon skill?"))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("An agent is running here. A short skill teaches agents xenon open so they can show a file beside this terminal instead of dumping it in chat."),
            )
            .child(
                div()
                    .mt_1()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(colors.background)
                    .border_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(colors.text_muted)
                    .font_family("ui-monospace")
                    .child("~/.agents/skills/xenon\n~/.claude/skills/xenon"),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(self.skill_btn("Don't ask again", BTN_NEVER, false, cx))
                    .child(self.skill_btn("Not now", BTN_LATER, false, cx))
                    .child(self.skill_btn("Install skill", BTN_INSTALL, true, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child("Return installs · Escape is Not now"),
            );
        div()
            .id("skill-prompt-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(scrim)
            .child(dialog)
    }

    fn skill_btn(
        &self,
        label: &'static str,
        index: usize,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let on = self.skill_prompt.is_some_and(|p| p.button == index);
        let bg = if primary {
            colors.text_accent
        } else if on {
            colors.element_selected
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
            .border_color(if on {
                colors.border_focused
            } else {
                colors.border
            })
            .bg(bg)
            .text_color(fg)
            .text_xs()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.pick_skill_button(index, cx)))
            .child(label)
    }
}
