//! Settings › Appearance: the mode, a strip of themes for each slot, and the
//! full picker (`prefs_picker.rs`).

use std::rc::Rc;

use adw::prelude::*;
use xenon_settings::{ThemeMode, swatch_names};

use crate::app::App;
use crate::prefs_picker::{lineup, set_current, tile};
use crate::theme;

/// True when the window shows its dark theme right now.
pub(crate) fn showing_dark(app: &App) -> bool {
    match app.settings.borrow().theme.mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => adw::StyleManager::default().is_dark(),
    }
}

/// After keeping a theme for the slot the window is not showing, say where it
/// will appear.
pub(crate) fn note_hidden_half(app: &App, dialog: &crate::prefs::Shell, kept_dark: bool) {
    let mode = app.settings.borrow().theme.mode;
    let shown = match mode {
        ThemeMode::Dark => Some(true),
        ThemeMode::Light => Some(false),
        ThemeMode::System => None,
    };
    let slot = if kept_dark { "dark" } else { "light" };
    let text = match shown {
        Some(dark) if dark == kept_dark => return,
        Some(_) => format!("Saved as the {slot} theme. Mode is not {slot}, so it shows later."),
        None if showing_dark(app) == kept_dark => return,
        None => format!("Saved as the {slot} theme. It shows when the desktop is {slot}."),
    };
    dialog.add_toast(&text);
}

/// Every theme name a tile on this page may show.
pub(crate) fn tile_names(app: &App) -> Vec<String> {
    let mut names: Vec<String> = lineup()
        .iter()
        .flat_map(|f| [f.dark, f.light])
        .flatten()
        .map(str::to_string)
        .collect();
    let theme = app.settings.borrow().theme.clone();
    names.extend([theme.dark, theme.light]);
    names.sort();
    names.dedup();
    names
}

pub(crate) fn page(app: &Rc<App>, dialog: &crate::prefs::Shell) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .name("appearance")
        .title("Appearance")
        .icon_name("applications-graphics-symbolic")
        .build();
    page.add(&mode_group(app));
    page.add(&slots_group(app, dialog));
    page
}

fn mode_group(app: &Rc<App>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title("Mode")
        .subtitle("System follows the desktop's light and dark")
        .build();
    let toggles = adw::ToggleGroup::new();
    toggles.set_valign(gtk::Align::Center);
    for (name, label) in [("system", "System"), ("light", "Light"), ("dark", "Dark")] {
        toggles.add(adw::Toggle::builder().name(name).label(label).build());
    }
    toggles.set_active_name(Some(match app.settings.borrow().theme.mode {
        ThemeMode::System => "system",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }));
    let weak = toggles.downgrade();
    crate::prefs_state::register("mode", move |v| {
        if let Some(t) = weak.upgrade() {
            t.set_active_name(Some(v));
        }
    });
    let app = app.clone();
    toggles.connect_active_name_notify(move |t| {
        let mode = match t.active_name().as_deref() {
            Some("light") => ThemeMode::Light,
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        app.change_settings(|s| s.theme.mode = mode);
    });
    row.add_suffix(&toggles);
    row.set_activatable_widget(Some(&toggles));
    group.add(&row);
    group
}

/// The two slot rows with their strips, and the row that opens the picker.
fn slots_group(app: &Rc<App>, dialog: &crate::prefs::Shell) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Theme")
        .description("Pick a theme for each side of the mode, or open them all.")
        .build();
    let slots: Rc<Vec<(bool, adw::ActionRow, gtk::Box)>> = Rc::new(
        [true, false]
            .into_iter()
            .map(|dark| {
                let row = adw::ActionRow::new();
                let strip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                strip.set_valign(gtk::Align::Center);
                row.add_suffix(&strip);
                group.add(&row);
                (dark, row, strip)
            })
            .collect(),
    );
    refill(app, &slots);
    let all = adw::ActionRow::builder()
        .title("All themes")
        .subtitle("Try each one in this window before keeping it")
        .activatable(true)
        .build();
    all.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let (app, dialog) = (app.clone(), dialog.clone());
    let on_keep: Rc<dyn Fn()> = {
        let (app, slots) = (app.clone(), slots.clone());
        Rc::new(move || refill(&app, &slots))
    };
    let open = Rc::new(move || {
        dialog.push(&crate::prefs_picker::page(&app, &dialog, on_keep.clone()));
    });
    let again = open.clone();
    all.connect_activated(move |_| again());
    crate::prefs_state::register("picker-open", move |_| open());
    group.add(&all);
    group
}

type Slots = Rc<Vec<(bool, adw::ActionRow, gtk::Box)>>;

/// Tooltips of the buttons in a strip, in order.
fn shown(strip: &gtk::Box) -> Vec<String> {
    let mut out = Vec::new();
    let mut child = strip.first_child();
    while let Some(widget) = child {
        out.push(widget.tooltip_text().map(String::from).unwrap_or_default());
        child = widget.next_sibling();
    }
    out
}

/// Bring both strips in line with the saved themes. A strip whose names did not
/// change keeps its buttons (and the focus on them); only the mark moves.
fn refill(app: &Rc<App>, slots: &Slots) {
    let saved = app.settings.borrow().theme.clone();
    for (dark, row, strip) in slots.iter() {
        let current = if *dark { &saved.dark } else { &saved.light };
        row.set_title(if *dark { "Dark theme" } else { "Light theme" });
        row.set_subtitle(current);
        let names: Vec<&str> = swatch_names(*dark, current)
            .into_iter()
            .filter(|n| theme::find(n).is_some())
            .collect();
        if shown(strip) != names {
            while let Some(child) = strip.first_child() {
                strip.remove(&child);
            }
            for name in &names {
                let button = tile(Some(name));
                button.set_focusable(true);
                let (app, slots, name, dark) =
                    (app.clone(), slots.clone(), name.to_string(), *dark);
                button.connect_clicked(move |_| {
                    app.keep_theme(&name, dark);
                    refill(&app, &slots);
                });
                strip.append(&button);
            }
        }
        let mut child = strip.first_child();
        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                set_current(
                    button,
                    button.tooltip_text().as_deref() == Some(current.as_str()),
                );
            }
            child = widget.next_sibling();
        }
    }
}
