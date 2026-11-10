use std::rc::Rc;

use adw::prelude::*;
use xenon_settings::{
    FontChoice, MAX_FONT_SIZE, MIN_FONT_SIZE, PROSE_FONTS, PROSE_LINE_HEIGHTS, PROSE_SIZES,
    ProseMarkers, Surface, TerminalAutoClose, ThemeMode, UiSettings, format_size,
    mono_font_choices, nearest_index,
};

use crate::app::App;
use crate::window::PangoProbe;

fn page(name: &str, title: &str, icon: &str) -> adw::PreferencesPage {
    adw::PreferencesPage::builder()
        .name(name)
        .title(title)
        .icon_name(icon)
        .build()
}

pub(crate) fn choice_row(
    title: &str,
    subtitle: &str,
    options: &[String],
    selected: usize,
    pick: impl Fn(usize) + 'static,
) -> adw::ComboRow {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk::StringList::new(&refs))
        .selected(selected as u32)
        .build();
    row.connect_selected_notify(move |row| pick(row.selected() as usize));
    row
}

pub(crate) fn switch_row(
    title: &str,
    subtitle: &str,
    on: bool,
    toggle: impl Fn(bool) + 'static,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(on)
        .build();
    let toggle = Rc::new(toggle);
    row.connect_active_notify(move |row| {
        let (toggle, on) = (toggle.clone(), row.is_active());
        gtk::glib::idle_add_local_once(move || toggle(on));
    });
    row
}

fn font_group(
    app: &Rc<App>,
    (title, detail): (&str, &str),
    surface: Surface,
    choices: Vec<FontChoice>,
    current: (&str, f32),
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(title)
        .description(detail)
        .build();
    let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
    let at = choices
        .iter()
        .position(|c| c.value == current.0)
        .unwrap_or(0);
    let family = {
        let app = app.clone();
        choice_row("Family", "", &labels, at, move |i| {
            let Some(choice) = choices.get(i) else { return };
            let probe = PangoProbe::new();
            app.change_settings(|s| s.set_font_family(surface, &choice.value, &probe));
        })
    };
    family.set_enable_search(true);
    family.set_expression(Some(&gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    )));
    group.add(&family);
    let size = adw::SpinRow::with_range(f64::from(MIN_FONT_SIZE), f64::from(MAX_FONT_SIZE), 1.0);
    size.set_title("Size");
    size.set_value(f64::from(current.1));
    let app = app.clone();
    size.connect_value_notify(move |row| {
        let (app, size) = (app.clone(), row.value() as f32);
        gtk::glib::idle_add_local_once(move || {
            app.change_settings(|s| s.set_font_size(surface, size))
        });
    });
    group.add(&size);
    group
}

pub(crate) fn appearance(app: &Rc<App>) -> adw::PreferencesPage {
    let page = page("appearance", "Appearance", "applications-graphics-symbolic");
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
    let app = app.clone();
    toggles.connect_active_name_notify(move |t| {
        let mode = match t.active_name().as_deref() {
            Some("light") => ThemeMode::Light,
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        let app = app.clone();
        gtk::glib::idle_add_local_once(move || app.change_settings(|s| s.theme.mode = mode));
    });
    row.add_suffix(&toggles);
    row.set_activatable_widget(Some(&toggles));
    group.add(&row);
    page.add(&group);
    page
}

pub(crate) fn fonts(app: &Rc<App>) -> adw::PreferencesPage {
    let page = page("fonts", "Fonts", "font-x-generic-symbolic");
    let probe = PangoProbe::new();
    let families = probe.0.list_families();
    let mono: Vec<String> = families
        .iter()
        .filter(|f| f.is_monospace())
        .map(|f| f.name().to_string())
        .collect();
    let ui = app.settings.borrow().clone();
    page.add(&font_group(
        app,
        ("Editor", "Monospaced faces only"),
        Surface::Editor,
        mono_font_choices(mono.clone(), &ui.editor.family),
        (&ui.editor.family, ui.editor.size),
    ));
    page.add(&font_group(
        app,
        ("Terminal", "Monospaced faces only"),
        Surface::Terminal,
        mono_font_choices(mono, &ui.terminal.family),
        (&ui.terminal.family, ui.terminal.size),
    ));
    page
}

fn flag(
    app: &Rc<App>,
    title: &str,
    detail: &str,
    on: bool,
    set: fn(&mut UiSettings, bool),
) -> adw::SwitchRow {
    let app = app.clone();
    switch_row(title, detail, on, move |on| {
        app.change_settings(|s| set(s, on))
    })
}

pub(crate) fn editor(app: &Rc<App>) -> adw::PreferencesPage {
    let page = page("editor", "Editor", "document-edit-symbolic");
    let s = app.settings.borrow().clone();
    let group = adw::PreferencesGroup::new();
    group.add(&flag(
        app,
        "Line numbers",
        "Show row numbers in text editors",
        s.show_line_numbers,
        |s, on| s.show_line_numbers = on,
    ));
    group.add(&flag(
        app,
        "Vim mode",
        "Modal editing in the text editor",
        s.vim_mode,
        |s, on| s.vim_mode = on,
    ));
    group.add(&flag(
        app,
        "Wrap prose",
        "Soft wrap Markdown and plain text. The file is not rewritten.",
        s.wrap_prose,
        |s, on| s.wrap_prose = on,
    ));
    group.add(&flag(
        app,
        "Wrap code",
        "Soft wrap every other file",
        s.wrap_code,
        |s, on| s.wrap_code = on,
    ));
    page.add(&group);
    page.add(&markdown(app, &s));
    page
}

fn numbers(values: &[f32]) -> Vec<String> {
    values.iter().map(|v| format_size(*v)).collect()
}

fn markdown(app: &Rc<App>, s: &UiSettings) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Markdown writing")
        .build();
    let p = &s.prose;
    let prose = |app: &Rc<App>, change: fn(&mut xenon_settings::ProseSettings, usize)| {
        let app = app.clone();
        move |i: usize| app.change_settings(|s| s.update_prose(|p| change(p, i)))
    };
    group.add(&choice_row(
        "Markers",
        "Dim keeps # * and [] visible as faint text; Hide draws only the result",
        &["Dim".into(), "Hide".into()],
        usize::from(p.markers == ProseMarkers::Hide),
        prose(app, |p, i| {
            p.markers = if i == 1 {
                ProseMarkers::Hide
            } else {
                ProseMarkers::Dim
            }
        }),
    ));
    group.add(&choice_row(
        "Writing font",
        "Auto is serif",
        &["Auto".into(), "Serif".into(), "Sans".into(), "Mono".into()],
        PROSE_FONTS.iter().position(|f| *f == p.font).unwrap_or(0),
        prose(app, |p, i| p.font = PROSE_FONTS[i]),
    ));
    group.add(&choice_row(
        "Writing size",
        "",
        &numbers(&PROSE_SIZES),
        nearest_index(&PROSE_SIZES, p.size),
        prose(app, |p, i| p.size = PROSE_SIZES[i]),
    ));
    group.add(&choice_row(
        "Line height",
        "",
        &numbers(&PROSE_LINE_HEIGHTS),
        nearest_index(&PROSE_LINE_HEIGHTS, p.line_height),
        prose(app, |p, i| p.line_height = PROSE_LINE_HEIGHTS[i]),
    ));
    group
}

pub(crate) fn terminal(app: &Rc<App>) -> adw::PreferencesGroup {
    let current = app.settings.borrow().terminal_auto_close;
    let labels: Vec<String> = ["Keep open", "Close", "1s", "3s", "5s"]
        .map(String::from)
        .into();
    let app = app.clone();
    let row = choice_row(
        "When the process exits",
        &new_tabs_detail(current),
        &labels,
        TerminalAutoClose::ALL
            .iter()
            .position(|m| *m == current)
            .unwrap_or(0),
        move |i| app.change_settings(|s| s.terminal_auto_close = TerminalAutoClose::ALL[i]),
    );
    row.connect_selected_notify(|row| {
        if let Some(mode) = TerminalAutoClose::ALL.get(row.selected() as usize) {
            let (row, mode) = (row.clone(), *mode);
            gtk::glib::idle_add_local_once(move || row.set_subtitle(&new_tabs_detail(mode)));
        }
    });
    let group = adw::PreferencesGroup::builder().title("Terminal").build();
    group.add(&row);
    group
}

fn new_tabs_detail(mode: TerminalAutoClose) -> String {
    format!("New terminal tabs: {}", mode.label().to_lowercase())
}
