use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use xenon_settings::{FontProbe, UiSettings};
use xenon_store::{AppSettings, ThemeMode, WindowState};

use crate::app::{App, defer};
use crate::commands::Cmd;
use crate::model::Model;
use crate::sidebar::Sidebar;
use crate::theme;

pub(crate) struct PangoProbe(pub(crate) gtk::pango::Context);

impl PangoProbe {
    pub(crate) fn new() -> Self {
        Self(gtk::Label::new(None).pango_context())
    }

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

pub(crate) fn mono_face(ui: &UiSettings, probe: &PangoProbe) -> String {
    if probe.is_monospace(&ui.terminal.family) {
        ui.terminal.family.clone()
    } else {
        "Monospace".into()
    }
}

impl App {
    pub(crate) fn apply_theme(&self) {
        let ui = self.settings.borrow().clone();
        adw::StyleManager::default().set_color_scheme(match ui.theme.mode {
            ThemeMode::System => adw::ColorScheme::Default,
            ThemeMode::Light => adw::ColorScheme::ForceLight,
            ThemeMode::Dark => adw::ColorScheme::ForceDark,
        });
        let sheet = format!(
            "{}\ntextview.xeditor, textview.xdiff-code {{ font-family: \"{}\", monospace; font-size: {}px; }}\n{}",
            theme::CSS,
            ui.editor.family,
            ui.editor.size,
            crate::prose::css(&ui.prose, &self.mono.borrow())
        );
        self.css.load_from_string(&sheet);
        for editor in self.editors.borrow().values() {
            editor.restyle(self);
        }
        let diffs: Vec<_> = self.diffs.borrow().values().cloned().collect();
        for view in diffs {
            view.rebuild();
        }
        for view in self.terms.borrow().values() {
            self.restyle_term(view);
        }
        xenon_phone::PhoneLink::set_dark(theme::dark());
    }
}

fn register_icons() {
    let bytes =
        gtk::glib::Bytes::from_static(include_bytes!(concat!(env!("OUT_DIR"), "/xenon.gresource")));
    if let Ok(resource) = gtk::gio::Resource::from_data(&bytes) {
        gtk::gio::resources_register(&resource);
    }
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path("/dev/xenon/gtk/icons");
    }
}

fn header(app: &Rc<App>, title: &adw::WindowTitle) -> adw::HeaderBar {
    let bar = adw::HeaderBar::new();
    bar.set_title_widget(Some(title));
    let button = |cmd: Cmd| {
        let b = gtk::Button::from_icon_name(cmd.icon());
        cmd.bind(&b);
        b.set_focusable(false);
        b
    };
    for cmd in [
        Cmd::ToggleSidebar,
        Cmd::PrevWorkspace,
        Cmd::NextWorkspace,
        Cmd::SplitRight,
        Cmd::SplitDown,
        Cmd::ParkPane,
        Cmd::Cut,
        Cmd::Copy,
        Cmd::CopyClean,
        Cmd::Paste,
    ] {
        bar.pack_start(&button(cmd));
    }
    bar.pack_end(&main_menu());
    bar.pack_end(&app.builds.button);
    // The save button sits between Settings and the rest, shown only while the editor is dirty.
    bar.pack_end(&button(Cmd::Settings));
    bar.pack_end(&app.save_button);
    for cmd in [
        Cmd::RunTask,
        Cmd::OpenWorklist,
        Cmd::CaptureWorklist,
        Cmd::OpenFinder,
        Cmd::NewTerminal,
    ] {
        bar.pack_end(&button(cmd));
    }
    bar
}

fn main_menu() -> gtk::MenuButton {
    let menu = gtk::gio::Menu::new();
    let leave = gtk::gio::Menu::new();
    leave.append_item(&Cmd::Quit.menu_item());
    leave.append_item(&Cmd::QuitEndTerminals.menu_item());
    menu.append_section(None, &leave);
    let more = gtk::gio::Menu::new();
    more.append_item(&Cmd::Settings.menu_item());
    more.append_item(&Cmd::ShowShortcuts.menu_item());
    menu.append_section(None, &more);
    let button = gtk::MenuButton::new();
    button.set_icon_name("open-menu-symbolic");
    button.set_menu_model(Some(&menu));
    button.set_tooltip_text(Some("Main Menu"));
    button.set_focusable(false);
    button
}

fn save_button() -> gtk::Button {
    let b = gtk::Button::from_icon_name("document-save-symbolic");
    b.set_action_name(Some(Cmd::Save.action()));
    b.set_focusable(false);
    b.set_visible(false);
    b
}

pub(crate) fn build(gtk_app: &adw::Application) -> Rc<App> {
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    register_icons();
    let settings: AppSettings = xenon_store::load_settings().unwrap_or_default();
    let probe = PangoProbe::new();
    let (ui, _) = UiSettings::resolve(&settings, &probe);
    let geometry = settings.window.filter(|g| g.is_sane());
    let (width, height) = geometry.map_or((1180, 760), |g| (g.width as i32, g.height as i32));
    let title = adw::WindowTitle::new("Xenon", "");
    let toasts = adw::ToastOverlay::new();
    let win = adw::ApplicationWindow::builder()
        .application(gtk_app)
        .title("Xenon")
        .default_width(width)
        .default_height(height)
        .maximized(geometry.is_some_and(|g| g.state == WindowState::Maximized))
        .build();
    let holders = xenon_term::Terminals::start(crate::term::child_env());
    let app = Rc::new_cyclic(|weak| App {
        gtk: gtk_app.clone(),
        win: win.clone(),
        model: RefCell::new(Model::load()),
        mono: RefCell::new(mono_face(&ui, &probe)),
        settings: RefCell::new(ui),
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
        images: RefCell::default(),
        diffs: RefCell::default(),
        git: crate::git::GitHub::new(weak.clone()),
        pulse_source: RefCell::new(None),
        pulse_level: Cell::new(-1),
        started: Instant::now(),
        tab_sync: Cell::new(false),
        transfer_queued: Cell::new(false),
        off_bar: Cell::new(false),
        save_pending: Cell::new(false),
        ended: RefCell::default(),
        save_button: save_button(),
        remote: crate::remote::Remote::new(),
        holders,
        builds: crate::builds_ui::Builds::new(),
        tasks: crate::tasks::Tasks::default(),
        trust_card: crate::tasks_ui::TrustCard::new(weak.clone()),
        worklist: Default::default(),
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
        defer(&weak, move |app| app.apply_theme());
    });

    crate::commands::install(&app);
    app.save_button.set_tooltip_text(Some(&Cmd::Save.tip()));
    app.stack
        .add_named(&crate::empty::all_clear(false), Some("none"));
    app.split.set_start_child(Some(&app.sidebar.root));
    app.split.set_end_child(Some(&app.stack));
    app.split.set_shrink_start_child(false);
    app.split.set_shrink_end_child(false);
    app.split.set_resize_start_child(false);
    app.split
        .set_position(settings.sidebar_width.clamp(140.0, 480.0) as i32);
    app.split.set_wide_handle(false);
    app.sidebar.restore_sizes(
        settings.workspaces_section_height,
        settings.files_section_height,
    );
    toasts.set_child(Some(&app.split));
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header(&app, &title));
    view.set_content(Some(&toasts));
    win.set_content(Some(&layered(&app, &view)));
    crate::git::listen(&app);
    connect_window(&app, &win);
    app.sidebar.set_hints();
    app.sidebar.rebuild(&app.model.borrow());
    app.listen_holders();
    app.refresh_builds();
    start_workspace(&app);
    app.remote_boot();
    app
}

fn layered(app: &App, view: &adw::ToolbarView) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(view));
    overlay.add_overlay(&app.trust_card.root);
    overlay
}

fn connect_window(app: &Rc<App>, win: &adw::ApplicationWindow) {
    let weak = Rc::downgrade(app);
    // sync: answer or ordering before return: the session must be saved before the window goes.
    win.connect_close_request(move |_| {
        if let Some(app) = weak.upgrade() {
            if app.close_hint_pending() {
                app.quit();
                return gtk::glib::Propagation::Stop;
            }
            app.save_all();
            app.save_window();
        }
        gtk::glib::Propagation::Proceed
    });
    let weak = Rc::downgrade(app);
    win.connect_is_active_notify(move |w| {
        let Some(app) = weak.upgrade() else { return };
        let active = w.is_active();
        app.model.borrow_mut().window_active = active;
        gtk::glib::idle_add_local_once(move || {
            if active {
                app.git_refresh_active();
            }
            app.dismiss_viewed();
            app.paint_all();
        });
    });
}

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
}
