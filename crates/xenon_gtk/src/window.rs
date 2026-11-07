//! Building the window: header bar, sidebar, content stack, theme, and the
//! first workspace.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::gio;
use xenon_settings::{FontProbe, UiSettings};
use xenon_store::{AppSettings, ThemeMode, WindowState};

use crate::app::App;
use crate::model::Model;
use crate::sidebar::Sidebar;
use crate::theme;

/// Answers font questions from Pango's installed families.
pub(crate) struct PangoProbe(pub(crate) gtk::pango::Context);

impl PangoProbe {
    fn family(&self, name: &str) -> Option<gtk::pango::FontFamily> {
        self.0
            .list_families()
            .into_iter()
            .find(|f| f.name().eq_ignore_ascii_case(name))
    }
}

impl FontProbe for PangoProbe {
    fn is_monospace(&self, family: &str) -> bool {
        self.family(family).is_some_and(|f| f.is_monospace())
    }

    fn resolves(&self, family: &str) -> bool {
        self.family(family).is_some()
    }
}

/// The terminal face to draw: the saved one when installed, else Monospace.
pub(crate) fn mono_face(ui: &UiSettings, probe: &PangoProbe) -> String {
    if probe.is_monospace(&ui.terminal.family) {
        ui.terminal.family.clone()
    } else {
        "Monospace".into()
    }
}

fn scheme(mode: ThemeMode) -> adw::ColorScheme {
    match mode {
        ThemeMode::System => adw::ColorScheme::Default,
        ThemeMode::Light => adw::ColorScheme::ForceLight,
        ThemeMode::Dark => adw::ColorScheme::ForceDark,
    }
}

impl App {
    /// Load the saved theme and restyle everything that shows it. A theme being
    /// previewed in Settings wins until the preview ends.
    pub(crate) fn apply_theme(&self) {
        let settings = self.settings.borrow().snapshot();
        let style = adw::StyleManager::default();
        let preview = crate::prefs_apply::previewing();
        let (palette, forced) = match preview
            .as_ref()
            .and_then(|(n, d)| Some((theme::find(n)?, *d)))
        {
            Some((palette, dark)) => (palette, Some(dark)),
            None => (theme::palette(&settings, style.is_dark()), None),
        };
        style.set_color_scheme(match forced {
            Some(true) => adw::ColorScheme::ForceDark,
            Some(false) => adw::ColorScheme::ForceLight,
            None => scheme(settings.theme),
        });
        let ui = self.settings.borrow().clone();
        let sheet = format!(
            "{}\n{}textview.xeditor {{ font-family: \"{}\", monospace; font-size: {}px; }}\n",
            palette.css(),
            crate::prefs_apply::extra_css(&ui.ui, &palette),
            ui.editor.family,
            ui.editor.size
        );
        self.css.load_from_string(&sheet);
        self.launcher
            .set_accent(&palette.color("text.accent", "#74ade8"));
        *self.palette.borrow_mut() = palette;
        for editor in self.editors.borrow().values() {
            editor.restyle(self);
        }
        for view in self.terms.borrow().values() {
            self.restyle_term(view);
        }
        self.remote_theme_changed();
        self.publish_settings();
    }

    fn tip(&self, text: &str, action: &str) -> String {
        match self.gtk.accels_for_action(action).first() {
            Some(accel) => match gtk::accelerator_parse(accel) {
                Some((key, mods)) => format!("{text} ({})", gtk::accelerator_get_label(key, mods)),
                None => text.to_string(),
            },
            None => text.to_string(),
        }
    }
}

fn header(app: &Rc<App>, title: &adw::WindowTitle) -> adw::HeaderBar {
    let bar = adw::HeaderBar::new();
    bar.set_title_widget(Some(title));
    let button = |icon: &str, action: &str, tip: String| {
        let b = gtk::Button::from_icon_name(icon);
        b.set_action_name(Some(action));
        b.set_tooltip_text(Some(&tip));
        b.set_focusable(false);
        b
    };
    bar.pack_start(&button(
        "sidebar-show-symbolic",
        "app.toggle-sidebar",
        app.tip("Toggle sidebar", "app.toggle-sidebar"),
    ));
    bar.pack_end(&button(
        "system-search-symbolic",
        "app.open-finder",
        app.tip("Find file", "app.open-finder"),
    ));
    bar.pack_end(&button(
        "view-dual-symbolic",
        "app.split-right",
        app.tip("Split right", "app.split-right"),
    ));
    bar.pack_end(&button(
        "view-paged-symbolic",
        "app.split-down",
        app.tip("Split down", "app.split-down"),
    ));
    bar
}

pub(crate) fn build(gtk_app: &adw::Application, user_keys: &[xenon_keymap::Entry]) -> Rc<App> {
    // Symbolic icons that exist in every install, whatever the desktop's theme.
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    let settings: AppSettings = xenon_store::load_settings().unwrap_or_default();
    let probe = PangoProbe(gtk::Label::new(None).pango_context());
    let (ui, _) = UiSettings::resolve(&settings, &probe);
    let geometry = settings.window.filter(|g| g.is_sane());
    let (width, height) = geometry.map_or((1180, 760), |g| (g.width as i32, g.height as i32));
    let title = adw::WindowTitle::new("Xenon", "");
    let toasts = adw::ToastOverlay::new();
    let win = adw::ApplicationWindow::builder()
        .application(gtk_app)
        .title("Xenon (GTK)")
        .default_width(width)
        .default_height(height)
        .maximized(geometry.is_some_and(|g| g.state == WindowState::Maximized))
        .build();
    let app = Rc::new_cyclic(|weak| App {
        gtk: gtk_app.clone(),
        win: win.clone(),
        model: RefCell::new(Model::load()),
        mono: RefCell::new(mono_face(&ui, &probe)),
        settings: RefCell::new(ui),
        palette: RefCell::new(theme::palette(&settings, false)),
        css: gtk::CssProvider::new(),
        sidebar: Sidebar::new(weak.clone()),
        split: gtk::Paned::new(gtk::Orientation::Horizontal),
        stack: gtk::Stack::new(),
        toasts: toasts.clone(),
        title: title.clone(),
        slots: RefCell::default(),
        terms: RefCell::default(),
        bodies: RefCell::new(HashMap::new()),
        launcher: crate::palette::Palette::new(),
        recents: RefCell::default(),
        file_indexes: RefCell::default(),
        editors: RefCell::default(),
        pulse_source: RefCell::new(None),
        pulse_level: Cell::new(-1),
        started: Instant::now(),
        save_pending: Cell::new(false),
        remote: crate::remote::Remote::new(),
    });
    gtk::style_context_add_provider_for_display(
        &gtk::prelude::WidgetExt::display(&win),
        &app.css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    app.apply_theme();
    let style = adw::StyleManager::default();
    let weak = Rc::downgrade(&app);
    style.connect_dark_notify(move |_| {
        if let Some(app) = weak.upgrade() {
            app.apply_theme();
        }
    });

    app.stack
        .add_named(&crate::empty::all_clear(), Some("none"));
    app.split.set_start_child(Some(&app.sidebar.root));
    app.split.set_end_child(Some(&app.stack));
    app.split.set_shrink_start_child(false);
    app.split.set_shrink_end_child(false);
    app.split.set_resize_start_child(false);
    app.split
        .set_position(settings.sidebar_width.clamp(140.0, 480.0) as i32);
    app.split.set_wide_handle(false);
    toasts.set_child(Some(&app.split));
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header(&app, &title));
    view.set_content(Some(&toasts));
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&view));
    overlay.add_overlay(&app.launcher.scrim);
    win.set_content(Some(&overlay));
    {
        let weak = Rc::downgrade(&app);
        app.launcher.set_on_change(move || {
            if let Some(app) = weak.upgrade() {
                app.publish_palette();
            }
        });
    }
    crate::latency::watch_frames(&win);

    crate::actions::install(&app, user_keys);
    connect_window(&app, &win);
    app.sidebar.rebuild(&app.model.borrow());
    app.sidebar.root.append(&app.remote.footer.root);
    start_workspace(&app);
    app.remote_boot();
    app
}

fn connect_window(app: &Rc<App>, win: &adw::ApplicationWindow) {
    let weak = Rc::downgrade(app);
    win.connect_close_request(move |_| {
        if let Some(app) = weak.upgrade() {
            app.save_all();
            app.save_window();
        }
        gtk::glib::Propagation::Proceed
    });
    let weak = Rc::downgrade(app);
    win.connect_is_active_notify(move |w| {
        let Some(app) = weak.upgrade() else { return };
        app.model.borrow_mut().window_active = w.is_active();
        app.model.borrow_mut().dismiss_viewed();
        app.paint_all();
    });
}

/// Show the workspace that was open last, or the first one, or the empty page.
fn start_workspace(app: &Rc<App>) {
    let first = {
        let model = app.model.borrow();
        model
            .active()
            .or_else(|| model.registry.workspaces.first().map(|r| r.id))
    };
    match first {
        Some(id) => app.activate_workspace(id),
        None => app.paint_all(),
    }
    let _ = gio::Application::default();
}
