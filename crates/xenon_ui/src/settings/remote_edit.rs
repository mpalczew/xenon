//! Phone remote address-override editing in Settings.

use gpui::{AppContext, Context, Entity, KeyDownEvent, Window};
use xenon_design_system::{TextInputAppearance, TextInputConfig, TextInputView};

use super::SettingsView;

pub(super) struct RemoteFieldEdit {
    pub(super) input: Entity<TextInputView>,
}

impl SettingsView {
    pub(super) fn begin_hostname_edit(
        &mut self,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_dropdown(window, cx);
        let placeholder = "e.g. macbook.tailnet.ts.net";
        let input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line(placeholder)
                    .appearance(TextInputAppearance::Inline)
                    .dialog_field(),
                cx,
            )
        });
        input.update(cx, |input, cx| {
            input.set_text(current, cx);
            input.open(cx);
        });
        self.remote_edit = Some(RemoteFieldEdit { input });
        cx.notify();
    }

    pub(super) fn commit_remote_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(RemoteFieldEdit { input }) = self.remote_edit.take() else {
            return;
        };
        let value = input.read(cx).text().to_owned();
        crate::app::remote::set_remote_hostname(value, window, cx);
        self.focus.focus(window, cx);
        window.refresh();
        cx.notify();
    }

    pub(super) fn cancel_remote_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remote_edit = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Parent-navigation keys bubble from the shared text control.
    pub(super) fn handle_remote_edit_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.remote_edit.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.cancel_remote_edit(window, cx),
            "enter" => self.commit_remote_edit(window, cx),
            _ => return false,
        }
        cx.stop_propagation();
        true
    }
}
