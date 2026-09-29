//! Settings › Appearance: mode, a swatch strip for the slot on screen, and the
//! other slot's theme. The full gallery stays on ⌘⌥T.

use std::rc::Rc;

use gpui::{App, Hsla, SharedString, Window};
use theme::{ActiveTheme, Appearance, ThemeRegistry};
use xenon_settings::ThemeMode;

use super::row::{ActionKind, Control, Group, RowAction, SettingRow, Swatch};

const DARK_PICKS: [&str; 5] = ["One Dark", "Neon Noir", "Nord", "Tokyo Night", "True Black"];
const LIGHT_PICKS: [&str; 5] = [
    "One Light",
    "Solarized Light",
    "Nord Light",
    "Tokyo Day",
    "Xcode Light",
];
const MODES: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

pub(super) fn groups(cx: &App) -> Vec<Group> {
    let settings = xenon_settings::snapshot(cx);
    let showing = showing_appearance(settings.theme, cx);
    let (slot, other, other_name) = match showing {
        Appearance::Dark => ("Dark theme", "Light theme", settings.light_theme.clone()),
        Appearance::Light => ("Light theme", "Dark theme", settings.dark_theme.clone()),
    };
    let current = slot_theme(&settings, showing);
    let selected = MODES.iter().position(|m| *m == settings.theme).unwrap_or(0);
    vec![Group::untitled(vec![
        SettingRow::new(
            "mode",
            "Mode",
            Control::Choice {
                options: vec!["System".into(), "Light".into(), "Dark".into()],
                selected,
                pick: Rc::new(|index, _, cx| apply_mode(MODES[index], cx)),
            },
        )
        .detail("System follows macOS light and dark")
        .keywords("appearance dark light system"),
        SettingRow::new(
            "theme-swatches",
            slot,
            Control::Swatches(swatches(showing, &current, cx)),
        )
        .detail("←/→ tries the next one · Enter opens every theme")
        .keywords("theme color palette"),
        SettingRow::new(
            "other-theme",
            other,
            Control::Action(RowAction {
                label: "All themes  ⌘⌥T".into(),
                kind: ActionKind::Quiet,
                run: Rc::new(open_theme_gallery),
            }),
        )
        .detail(other_name)
        .keywords("theme color"),
    ])]
}

fn swatches(appearance: Appearance, current: &str, cx: &App) -> Vec<Swatch> {
    let picks = match appearance {
        Appearance::Dark => DARK_PICKS,
        Appearance::Light => LIGHT_PICKS,
    };
    let mut names: Vec<&str> = picks.to_vec();
    // Keep the live theme visible even when it is not a curated pick.
    if !names.contains(&current) {
        names.pop();
        names.insert(0, current);
    }
    names
        .into_iter()
        .filter_map(|name| swatch(name, appearance, name == current, cx))
        .collect()
}

fn swatch(name: &str, appearance: Appearance, current: bool, cx: &App) -> Option<Swatch> {
    let theme = ThemeRegistry::global(cx).get(name).ok()?;
    let colors = theme.colors();
    let keyword = syntax_color(&theme, "keyword").unwrap_or(colors.text);
    Some(Swatch {
        name: SharedString::from(name.to_string()),
        colors: [colors.editor_background, colors.text_accent, keyword],
        current,
        appearance,
    })
}

pub(super) fn syntax_color(theme: &theme::Theme, name: &str) -> Option<Hsla> {
    theme
        .syntax()
        .style_for_name(name)
        .and_then(|style| style.color)
}

/// ←/→ on the strip: apply the neighbouring swatch.
pub(super) fn step_swatch(swatches: &[Swatch], forward: bool, cx: &mut App) {
    let Some(current) = swatches.iter().position(|s| s.current) else {
        return;
    };
    let next = if forward {
        (current + 1).min(swatches.len() - 1)
    } else {
        current.saturating_sub(1)
    };
    if next != current {
        let pick = &swatches[next];
        crate::theme_picker::apply_named_theme(&pick.name, pick.appearance, cx);
    }
}

pub(super) fn pick_swatch(swatch: &Swatch, cx: &mut App) {
    crate::theme_picker::apply_named_theme(&swatch.name, swatch.appearance, cx);
}

fn slot_theme(settings: &xenon_store::AppSettings, appearance: Appearance) -> String {
    match appearance {
        Appearance::Light => settings.light_theme.clone(),
        Appearance::Dark => settings.dark_theme.clone(),
    }
}

fn showing_appearance(mode: ThemeMode, cx: &App) -> Appearance {
    match mode {
        ThemeMode::Light => Appearance::Light,
        ThemeMode::Dark => Appearance::Dark,
        ThemeMode::System => theme::SystemAppearance::global(cx).0,
    }
}

fn apply_mode(mode: ThemeMode, cx: &mut App) {
    let mut settings = xenon_settings::snapshot(cx);
    settings.theme = mode;
    xenon_settings::apply(&settings, cx);
    xenon_settings::save(cx);
    xenon_terminal::apply_theme(cx);
}

pub(super) fn open_theme_gallery(_window: &mut Window, cx: &mut App) {
    for handle in cx.windows() {
        let Some(main) = handle.downcast::<crate::XenonApp>() else {
            continue;
        };
        let _ = main.update(cx, |app, window, cx| {
            app.open_theme_picker(window, cx);
            window.activate_window();
        });
    }
}

/// The live preview reads syntax colors from the active theme.
pub(super) fn active_syntax(cx: &App, name: &str) -> Hsla {
    syntax_color(cx.theme(), name).unwrap_or(cx.theme().colors().text)
}
