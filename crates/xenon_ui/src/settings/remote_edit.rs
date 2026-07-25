//! Remote hostname/password inline edit control for Settings.

use std::time::Duration;

use gpui::{Context, KeyDownEvent, Window};

use super::SettingsView;
use super::line_edit::LineEdit;

const CARET_BLINK: Duration = Duration::from_millis(530);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RemoteEditField {
    Password,
    Hostname,
}

pub(super) struct RemoteFieldEdit {
    pub(super) field: RemoteEditField,
    pub(super) edit: LineEdit,
}

impl SettingsView {
    pub(super) fn begin_remote_edit(
        &mut self,
        field: RemoteEditField,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_dropdown(cx);
        self.remote_edit = Some(RemoteFieldEdit {
            field,
            edit: LineEdit::new(current),
        });
        self.caret_on = true;
        self.focus.focus(window, cx);
        self.start_remote_edit_caret_blink(cx);
        cx.notify();
    }

    pub(super) fn begin_password_edit(
        &mut self,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_remote_edit(RemoteEditField::Password, current, window, cx);
    }

    pub(super) fn begin_hostname_edit(
        &mut self,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_remote_edit(RemoteEditField::Hostname, current, window, cx);
    }

    pub(super) fn commit_remote_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(RemoteFieldEdit { field, edit }) = self.remote_edit.take() else {
            return;
        };
        self._blink = None;
        let value = edit.into_text();
        match field {
            RemoteEditField::Password => {
                crate::app::remote::set_remote_password(value, window, cx);
            }
            RemoteEditField::Hostname => {
                crate::app::remote::set_remote_hostname(value, cx);
            }
        }
        window.refresh();
        cx.notify();
    }

    pub(super) fn cancel_remote_edit(&mut self, cx: &mut Context<Self>) {
        self.remote_edit = None;
        self._blink = None;
        cx.notify();
    }

    fn start_remote_edit_caret_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        self._blink = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CARET_BLINK).await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.remote_edit.is_some() {
                            this.caret_on = !this.caret_on;
                            cx.notify();
                            true
                        } else {
                            this.caret_on = true;
                            this._blink = None;
                            false
                        }
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }

    /// Handle keys while remote hostname/password edit is open.
    /// Returns true when the event was for remote edit (caller should return).
    pub(super) fn handle_remote_edit_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.remote_edit.is_none() {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;
        let extend = mods.shift;
        if (mods.platform || mods.control) && key == "a" {
            if let Some(re) = self.remote_edit.as_mut() {
                re.edit.select_all();
                self.caret_on = true;
                cx.notify();
            }
            cx.stop_propagation();
            return true;
        }
        match key {
            "escape" => {
                self.cancel_remote_edit(cx);
                cx.stop_propagation();
            }
            "enter" => {
                self.commit_remote_edit(window, cx);
                cx.stop_propagation();
            }
            "backspace" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.backspace();
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "delete" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.delete_forward();
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "left" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.move_left(extend);
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "right" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.move_right(extend);
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "up" | "home" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.home(extend);
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "down" | "end" => {
                if let Some(re) = self.remote_edit.as_mut() {
                    re.edit.end(extend);
                    self.caret_on = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            _ => {}
        }
        true
    }
}
