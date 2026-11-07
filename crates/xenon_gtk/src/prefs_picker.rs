//! The theme picker page of Settings: one row per family with a dark and a
//! light tile. Moving the selection re-themes the whole window live; Enter or
//! Keep saves it in its slot, Escape or Back puts back what was showing. The
//! lineup and the dark/light rules come from `xenon_settings`.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_settings::{THEME_LINEUP, ThemeFamily};

use crate::app::App;
use crate::theme;

/// Families whose theme files are bundled (a missing file drops that half).
pub(crate) fn lineup() -> Vec<ThemeFamily> {
    let present = |name: Option<&'static str>| name.filter(|n| theme::find(n).is_some());
    THEME_LINEUP
        .iter()
        .map(|f| ThemeFamily {
            name: f.name,
            group: f.group,
            dark: present(f.dark),
            light: present(f.light),
        })
        .filter(|f| f.dark.is_some() || f.light.is_some())
        .collect()
}

/// A CSS class that is safe for any theme name.
pub(crate) fn slug(name: &str) -> String {
    let body: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("xt-{body}")
}

/// The sheet that paints each tile in its own theme's colors.
pub(crate) fn tile_css(names: &[String]) -> String {
    let mut sheet = String::new();
    for name in names {
        let Some(p) = theme::find(name) else { continue };
        let class = slug(name);
        let kw = p
            .syntax("keyword")
            .unwrap_or_else(|| p.color("text", "#888"));
        sheet += &format!(
            "button.xtile.{class} {{ background: {bg}; color: {fg}; border: 1px solid {border}; }}\n\
             button.xtile.{class} .xa {{ color: {accent}; font-weight: 700; }}\n\
             button.xtile.{class} .xk {{ color: {kw}; }}\n",
            bg = p.color("editor.background", "#000000"),
            fg = p.color("text", "#cccccc"),
            border = p.color("border", "#444444"),
            accent = p.color("text.accent", "#74ade8"),
        );
    }
    sheet
}

/// A swatch button: sample text in the theme's colors.
pub(crate) fn tile(name: Option<&str>) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("xtile");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    content.set_halign(gtk::Align::Center);
    let aa = gtk::Label::new(Some("Aa"));
    aa.add_css_class("xa");
    let kw = gtk::Label::new(Some("fn"));
    kw.add_css_class("xk");
    content.append(&aa);
    content.append(&kw);
    match name {
        Some(name) => {
            button.add_css_class(&slug(name));
            button.set_tooltip_text(Some(name));
            button.update_property(&[gtk::accessible::Property::Label(name)]);
        }
        None => button.set_sensitive(false),
    }
    button.set_child(Some(&content));
    button.set_focusable(false);
    button
}

struct State {
    families: Vec<ThemeFamily>,
    rows: Vec<(gtk::Button, gtk::Button)>,
    selected: Cell<usize>,
    dark: Cell<bool>,
    kept: Cell<bool>,
    quiet: Cell<bool>,
}

impl State {
    fn mark(&self) {
        for (i, (dark, light)) in self.rows.iter().enumerate() {
            let on = i == self.selected.get();
            set_current(dark, on && self.dark.get());
            set_current(light, on && !self.dark.get());
        }
    }
}

pub(crate) fn set_current(button: &gtk::Button, on: bool) {
    if on {
        button.add_css_class("current");
    } else {
        button.remove_css_class("current");
    }
}

type Tiles = Vec<(gtk::Button, gtk::Button)>;

/// The list with one row per family, and each row's dark and light tile.
fn build_list(
    families: &[ThemeFamily],
    saved: &xenon_settings::ThemePreference,
) -> (gtk::ListBox, Tiles) {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Browse);
    list.add_css_class("navigation-sidebar");
    let mut rows = Vec::new();
    let mut groups = Vec::new();
    for family in families {
        let (dark, light) = (tile(family.dark), tile(family.light));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(8);
        row.set_margin_end(8);
        let name = gtk::Label::new(Some(family.name));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        row.append(&name);
        if family.contains(&saved.dark) || family.contains(&saved.light) {
            let badge = gtk::Label::new(Some("in use"));
            badge.set_css_classes(&["caption-heading", "dim-label"]);
            row.append(&badge);
        }
        row.append(&dark);
        row.append(&light);
        list.append(&row);
        rows.push((dark, light));
        groups.push(family.group);
    }
    list.set_header_func(move |row, before| {
        let group = groups.get(row.index() as usize).copied();
        let previous = before.and_then(|b| groups.get(b.index() as usize).copied());
        if group != previous {
            let label = gtk::Label::new(group.map(str::to_uppercase).as_deref());
            label.set_xalign(0.0);
            label.add_css_class("xheading");
            label.set_margin_top(10);
            label.set_margin_start(8);
            row.set_header(Some(&label));
        } else {
            row.set_header(None::<&gtk::Widget>);
        }
    });
    (list, rows)
}

/// Left picks the dark half, Right the light half.
fn add_half_keys(
    list: &gtk::ListBox,
    select: &Rc<impl Fn(usize, bool) + 'static>,
    state: &Rc<State>,
) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let (select, state) = (select.clone(), state.clone());
        keys.connect_key_pressed(move |_, key, _, _| match key {
            gtk::gdk::Key::Left => {
                select(state.selected.get(), true);
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Right => {
                select(state.selected.get(), false);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
    }
    list.add_controller(keys);
}

/// Select the live theme once the page is up; revert a preview when it goes.
fn on_page_events(
    page: &adw::NavigationPage,
    state: &Rc<State>,
    list: &gtk::ListBox,
    app: &Rc<App>,
) {
    {
        let (state, list) = (state.clone(), list.clone());
        page.connect_showing(move |_| {
            // Let the page settle first: focusing a row while it appears makes
            // the list move its selection one row down.
            let (state, list) = (state.clone(), list.clone());
            glib::idle_add_local_once(move || {
                if let Some(row) = list.row_at_index(state.selected.get() as i32) {
                    list.select_row(Some(&row));
                    row.grab_focus();
                }
                state.mark();
                glib::idle_add_local_once(move || state.quiet.set(false));
            });
        });
    }
    {
        let (state, app) = (state.clone(), app.clone());
        page.connect_hidden(move |_| {
            state.quiet.set(true);
            if !state.kept.get() {
                app.end_preview();
            }
            state.kept.set(false);
        });
    }
}

pub(crate) fn page(
    app: &Rc<App>,
    dialog: &crate::prefs::Shell,
    on_keep: Rc<dyn Fn()>,
) -> adw::NavigationPage {
    let families = lineup();
    let saved = app.settings.borrow().theme.clone();
    let showing_dark = crate::prefs_theme::showing_dark(app);
    let live = if showing_dark {
        &saved.dark
    } else {
        &saved.light
    };
    let (list, rows) = build_list(&families, &saved);
    let state = Rc::new(State {
        selected: Cell::new(families.iter().position(|f| f.contains(live)).unwrap_or(0)),
        dark: Cell::new(showing_dark),
        families,
        rows,
        kept: Cell::new(false),
        quiet: Cell::new(true),
    });
    let select = {
        let (state, app, list) = (state.clone(), app.clone(), list.clone());
        Rc::new(move |index: usize, dark: bool| {
            let Some(family) = state.families.get(index) else {
                return;
            };
            let (dark, name) = family.nearest(dark);
            state.selected.set(index);
            state.dark.set(dark);
            if let Some(row) = list.row_at_index(index as i32)
                && !row.is_selected()
            {
                list.select_row(Some(&row));
            }
            state.mark();
            if !state.quiet.get() {
                app.preview_theme(name, dark);
            }
        })
    };
    {
        let (select, state) = (select.clone(), state.clone());
        crate::prefs_state::register("picker-select", move |v| {
            let Some((name, half)) = v.split_once(':') else {
                return;
            };
            if let Some(i) = state.families.iter().position(|f| f.name == name) {
                select(i, half == "dark");
            }
        });
    }
    for (i, (dark, light)) in state.rows.iter().enumerate() {
        for (button, half) in [(dark, true), (light, false)] {
            let select = select.clone();
            button.connect_clicked(move |_| select(i, half));
        }
    }
    {
        let (select, state) = (select.clone(), state.clone());
        list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                select(row.index() as usize, state.dark.get());
            }
        });
    }
    let keep = {
        let (state, app, dialog) = (state.clone(), app.clone(), dialog.clone());
        Rc::new(move || {
            let Some(family) = state.families.get(state.selected.get()) else {
                return;
            };
            let (dark, name) = family.nearest(state.dark.get());
            state.kept.set(true);
            app.keep_theme(name, dark);
            on_keep();
            crate::prefs_theme::note_hidden_half(&app, &dialog, dark);
            dialog.pop();
        })
    };
    {
        let keep = keep.clone();
        list.connect_row_activated(move |_, _| keep());
    }
    {
        let keep = keep.clone();
        crate::prefs_state::register("picker-keep", move |_| keep());
        let dialog = dialog.clone();
        crate::prefs_state::register("picker-revert", move |_| {
            dialog.pop();
        });
    }
    add_half_keys(&list, &select, &state);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&list));
    scroller.set_vexpand(true);
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&scroller));
    view.add_bottom_bar(&bottom_bar(dialog, &keep));
    let page = adw::NavigationPage::new(&view, "Themes");
    on_page_events(&page, &state, &list, app);
    page
}

fn bottom_bar(dialog: &crate::prefs::Shell, keep: &Rc<impl Fn() + 'static>) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.set_margin_top(8);
    bar.set_margin_bottom(8);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    let hint = gtk::Label::new(Some("↑↓ theme · ←→ dark/light"));
    hint.add_css_class("dim-label");
    hint.set_hexpand(true);
    hint.set_xalign(0.0);
    bar.append(&hint);
    let revert = gtk::Button::with_label("Revert  esc");
    let dialog = dialog.clone();
    revert.connect_clicked(move |_| {
        dialog.pop();
    });
    let keep_button = gtk::Button::with_label("Keep  \u{21b5}");
    keep_button.add_css_class("suggested-action");
    let keep = keep.clone();
    keep_button.connect_clicked(move |_| keep());
    bar.append(&revert);
    bar.append(&keep_button);
    bar
}
