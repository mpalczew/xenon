//! Settings window scenes: one page each and search results.

use gpui::{Context, Window};

use super::{Scene, XenonApp, populate};
use crate::dropdown::DropdownId;
use crate::settings::SettingsPage;

pub(super) fn page(app: &mut XenonApp, scene: Scene, cx: &mut Context<XenonApp>) {
    let (page, query) = match scene {
        Scene::SettingsEditor => (SettingsPage::Editor, ""),
        Scene::SettingsLanguageServers => (SettingsPage::LanguageServers, ""),
        // A broad query: results must overflow and scroll, not squash.
        Scene::SettingsSearch => (SettingsPage::Appearance, "k"),
        _ => (SettingsPage::Appearance, ""),
    };
    show(app, page, query, cx);
}

pub(super) fn dropdown(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    show(app, SettingsPage::Fonts, "", cx);
    open_dropdown(app, DropdownId::Editor, "", cx);
}

fn open_dropdown(app: &XenonApp, id: DropdownId, filter: &'static str, cx: &mut Context<XenonApp>) {
    with_settings(app, cx, move |settings, window, cx| {
        settings.toggle_dropdown(id, window, cx);
        settings.visual_filter(filter, cx);
    });
}

pub(super) fn agents_installed(app: &mut XenonApp, cx: &mut Context<XenonApp>) {
    let _ = xenon_store::install_skill();
    show(app, SettingsPage::Agents, "", cx);
}

pub(super) fn remote(app: &mut XenonApp, cx: &mut Context<XenonApp>) {
    app.visual_remote(true, cx);
    show(app, SettingsPage::PhoneRemote, "", cx);
}

fn show(app: &mut XenonApp, page: SettingsPage, query: &'static str, cx: &mut Context<XenonApp>) {
    crate::settings::pin_version_for_visuals(1);
    app.open_settings_window_now(cx);
    with_settings(app, cx, move |settings, window, cx| {
        settings.visual_show(page, query, window, cx);
    });
}

fn with_settings(
    app: &XenonApp,
    cx: &mut Context<XenonApp>,
    f: impl FnOnce(&mut crate::SettingsView, &mut Window, &mut Context<crate::SettingsView>),
) {
    if let Some(handle) = app.settings_window {
        let _ = handle.update(cx, f);
    }
}
