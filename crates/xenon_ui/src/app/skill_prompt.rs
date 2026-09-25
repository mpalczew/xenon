//! In-app "Install the Xenon skill?" dialog. Keyboard: Return / Escape / arrows.

use super::*;
use xenon_design_system::FocusOnOpen;
use xenon_store::{decline_skill, install_skill, should_prompt_skill};

const BTN_NEVER: usize = 0;
const BTN_LATER: usize = 1;
const BTN_INSTALL: usize = 2;

pub(crate) struct SkillPrompt {
    pub button: usize,
    focus: FocusOnOpen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SkillKey {
    Confirm,
    Later,
    Move(isize),
}

/// Map a keystroke to a prompt action. The dialog owns focus, so these keys
/// must not go to the terminal.
pub(super) fn interpret_skill_key(key: &str) -> Option<SkillKey> {
    match key {
        "escape" => Some(SkillKey::Later),
        "enter" | "return" => Some(SkillKey::Confirm),
        "left" => Some(SkillKey::Move(-1)),
        "right" => Some(SkillKey::Move(1)),
        _ => None,
    }
}

impl XenonApp {
    pub(crate) fn maybe_offer_skill(&mut self, title: &str, cx: &mut Context<Self>) {
        if self.skill_prompt.is_some() || self.skill_skipped_session {
            return;
        }
        if self.finder.is_some() || self.command_palette.is_some() {
            return;
        }
        if !xenon_terminal::agent_title(title) {
            return;
        }
        if !should_prompt_skill() {
            return;
        }
        self.begin_skill_prompt(cx);
        self.deferred.restore_pane = Some(FocusOwner::Terminal);
        cx.notify();
    }

    #[cfg(feature = "visual-tests")]
    pub(crate) fn show_skill_prompt(&mut self, cx: &mut Context<Self>) {
        self.begin_skill_prompt(cx);
        cx.notify();
    }

    fn begin_skill_prompt(&mut self, cx: &mut Context<Self>) {
        let mut focus = FocusOnOpen::new(cx.focus_handle());
        focus.open();
        self.skill_prompt = Some(SkillPrompt {
            button: BTN_INSTALL,
            focus,
        });
    }

    pub(super) fn focus_skill_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.skill_prompt.as_mut() else {
            return;
        };
        prompt.focus.focus_after_open(window, cx);
    }

    pub(crate) fn on_skill_prompt_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.skill_prompt.is_none() {
            return false;
        }
        let Some(action) = interpret_skill_key(event.keystroke.key.as_str()) else {
            return false;
        };
        match action {
            SkillKey::Later => self.dismiss_skill_later(cx),
            SkillKey::Confirm => self.confirm_skill_prompt(cx),
            SkillKey::Move(delta) => {
                if let Some(prompt) = self.skill_prompt.as_mut() {
                    let next = prompt.button as isize + delta;
                    prompt.button = next.clamp(0, BTN_INSTALL as isize) as usize;
                    cx.notify();
                }
            }
        }
        true
    }

    fn confirm_skill_prompt(&mut self, cx: &mut Context<Self>) {
        let button = self
            .skill_prompt
            .as_ref()
            .map(|p| p.button)
            .unwrap_or(BTN_INSTALL);
        match button {
            BTN_NEVER => {
                let _ = decline_skill();
                self.close_skill_prompt(true, cx);
            }
            BTN_LATER => self.dismiss_skill_later(cx),
            _ => {
                if let Err(error) = install_skill() {
                    log::error!("skill install: {error}");
                }
                self.close_skill_prompt(true, cx);
            }
        }
    }

    fn dismiss_skill_later(&mut self, cx: &mut Context<Self>) {
        self.close_skill_prompt(true, cx);
    }

    fn close_skill_prompt(&mut self, restore: bool, cx: &mut Context<Self>) {
        self.skill_prompt = None;
        self.skill_skipped_session = true;
        if restore {
            self.deferred.pending_focus = self
                .deferred
                .restore_pane
                .take()
                .or(Some(FocusOwner::Terminal));
        }
        cx.notify();
    }

    fn pick_skill_button(&mut self, button: usize, cx: &mut Context<Self>) {
        if let Some(prompt) = self.skill_prompt.as_mut() {
            prompt.button = button;
        }
        self.confirm_skill_prompt(cx);
    }

    pub(crate) fn render_skill_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let scrim = colors.background.opacity(0.55);
        let focus = self
            .skill_prompt
            .as_ref()
            .map(|p| p.focus.handle())
            .unwrap_or_else(|| cx.focus_handle());
        let dialog = div()
            .id("skill-prompt-dialog")
            .occlude()
            .track_focus(&focus)
            .key_context("SkillPrompt")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if this.on_skill_prompt_key(event, cx) {
                    cx.stop_propagation();
                }
            }))
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
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Install the Xenon skill?"),
            )
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
        let on = self
            .skill_prompt
            .as_ref()
            .is_some_and(|p| p.button == index);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_and_escape_are_prompt_actions() {
        assert_eq!(interpret_skill_key("enter"), Some(SkillKey::Confirm));
        assert_eq!(interpret_skill_key("return"), Some(SkillKey::Confirm));
        assert_eq!(interpret_skill_key("escape"), Some(SkillKey::Later));
        assert_eq!(interpret_skill_key("left"), Some(SkillKey::Move(-1)));
        assert_eq!(interpret_skill_key("right"), Some(SkillKey::Move(1)));
        assert_eq!(interpret_skill_key("a"), None);
    }
}
