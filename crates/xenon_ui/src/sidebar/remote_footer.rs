//! Sidebar footer while the phone remote is on: listening, connected, or a
//! phone driving a terminal. Click (or ⌘⇧M) opens Connect Phone.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

use crate::app::{RemoteFooter, XenonApp};
use crate::icons::icon;

impl XenonApp {
    pub(super) fn render_remote_footer(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let status = self.remote_footer(cx)?;
        let colors = cx.theme().colors().clone();
        let (label, active) = match &status {
            RemoteFooter::Listening => ("Phone remote · listening".to_string(), false),
            RemoteFooter::Connected { device } => (format!("{device} connected"), true),
            RemoteFooter::Driving { device, workspace } => {
                (format!("{device} driving · {workspace}"), true)
            }
        };
        let tint = if active {
            colors.text_accent
        } else {
            colors.text_muted
        };
        Some(
            div()
                .id("remote-footer")
                .flex_none()
                .mt_auto()
                .flex()
                .items_center()
                .gap_2()
                .h(px(32.))
                .px_3()
                .border_t_1()
                .border_color(colors.border)
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(tint)
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover))
                .on_click(cx.listener(|this, _, _, cx| this.open_connect_phone(cx)))
                .child(icon(Icon::Smartphone, px(14.)))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .child(if active {
                    div().size(px(7.)).rounded_full().bg(colors.text_accent)
                } else {
                    div().text_color(colors.text_muted).child("⌘⇧M")
                }),
        )
    }
}
