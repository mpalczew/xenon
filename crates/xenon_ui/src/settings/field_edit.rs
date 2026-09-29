//! Inline editing for text settings (phone address, language server commands).

use gpui::{AppContext, Context, Entity, KeyDownEvent, Window};
use xenon_design_system::{TextInputAppearance, TextInputConfig, TextInputView};

use super::SettingsView;
use super::row::Field;

pub(super) struct FieldEdit {
    pub(super) field: Field,
    pub(super) input: Entity<TextInputView>,
}

fn placeholder(field: Field) -> &'static str {
    match field {
        Field::RemoteHostname => "e.g. macbook.tailnet.ts.net",
        Field::RustServer => "rust-analyzer",
        Field::TypeScriptServer => "typescript-language-server",
    }
}

impl SettingsView {
    pub(super) fn begin_field_edit(
        &mut self,
        field: Field,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_dropdown(window, cx);
        let input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line(placeholder(field))
                    .appearance(TextInputAppearance::Inline)
                    .dialog_field(),
                cx,
            )
        });
        input.update(cx, |input, cx| {
            input.set_text(current, cx);
            input.open(cx);
        });
        self.field_edit = Some(FieldEdit { field, input });
        cx.notify();
    }

    fn commit_field_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(FieldEdit { field, input }) = self.field_edit.take() else {
            return;
        };
        let value = input.read(cx).text().to_owned();
        match field {
            Field::RemoteHostname => crate::app::remote::set_remote_hostname(value, window, cx),
            Field::RustServer | Field::TypeScriptServer => {
                super::pages::set_server_command(field, value, cx)
            }
        }
        self.focus.focus(window, cx);
        window.refresh();
        cx.notify();
    }

    fn cancel_field_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field_edit = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Enter / Escape / Tab bubble from the dialog field.
    pub(super) fn handle_field_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.field_edit.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.cancel_field_edit(window, cx),
            "enter" | "tab" => self.commit_field_edit(window, cx),
            _ => return false,
        }
        cx.stop_propagation();
        true
    }
}
