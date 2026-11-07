//! Phone page appearance: which theme its colors resolve against.

use xenon_remote::{AppearanceWire, ServerMsg, ThemeWire};

use super::*;

impl XenonApp {
    /// The page chose dark/light/follow-Mac: tell it the colors and repaint
    /// the feed (new colors intern as new styles, so changed rows resend).
    pub(super) fn remote_appearance(
        &mut self,
        conn: ConnId,
        appearance: Option<AppearanceWire>,
        cx: &mut Context<Self>,
    ) {
        let appearance = appearance.map(|a| match a {
            AppearanceWire::Dark => theme::Appearance::Dark,
            AppearanceWire::Light => theme::Appearance::Light,
        });
        let resolved = xenon_terminal::remote_theme_for(appearance, cx);
        let Some(c) = self.conn_mut(conn) else {
            return;
        };
        c.appearance = appearance;
        c.out.try_send(ServerMsg::Theme {
            theme: theme_wire(&resolved),
        });
        if let Some(feed) = c.attach.as_ref().and_then(|a| a.feed.as_ref()) {
            feed.io().set_theme(resolved);
            feed.poke();
        }
    }
}

pub(super) fn theme_wire(theme: &theme::Theme) -> ThemeWire {
    xenon_remote::theme_wire(xenon_terminal::remote_theme_colors(theme))
}
