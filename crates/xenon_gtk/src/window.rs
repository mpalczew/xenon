//! Building the window: header bar, sidebar, content stack, theme, and the
//! first workspace.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::gio;
use xenon_settings::{FontProbe, UiSettings};
use xenon_store::{AppSettings, WindowState};

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

impl App {
    /// Restyle everything that follows the desktop's light or dark, and the
    /// settings that show in the sheet.
    pub(crate) fn apply_theme(&self) {
        let ui = self.settings.borrow().clone();
        let sheet = format!(
            "{}\n{}textview.xeditor, textview.xdiff-code {{ font-family: \"{}\", monospace; font-size: {}px; }}\n{}",
            theme::CSS,
            crate::prefs_apply::extra_css(&ui.ui),
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
        self.remote_theme_changed();
        self.publish_settings();
    }

    /// The first key bound to `action` as text (`Ctrl+Shift+O`), from the
    /// keymap in force (defaults plus the user's `keymap.json`).
    pub(crate) fn chord(&self, action: &str) -> Option<String> {
        let accel = self.gtk.accels_for_action(action).into_iter().next()?;
        let (key, mods) = gtk::accelerator_parse(&accel)?;
        Some(gtk::accelerator_get_label(key, mods).to_string())
    }

    /// Tooltip text in the GPUI app's form: `Label · chord`.
    pub(crate) fn tip(&self, text: &str, action: &str) -> String {
        match self.chord(action) {
            Some(chord) => format!("{text} \u{b7} {chord}"),
            None => text.to_string(),
        }
    }
}

/// Icons Adwaita lacks, compiled in by build.rs.
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
    let button = |icon: &str, action: &str, tip: String| {
        let b = gtk::Button::from_icon_name(icon);
        b.set_action_name(Some(action));
        b.set_tooltip_text(Some(&tip));
        b.set_focusable(false);
        b
    };
    // GPUI toolbar order (xenon_ui/src/toolbar.rs), left to right. Buttons for
    // commands GTK does not have yet (Go Back, Go Forward, Cut,
    // File Browser, Save) are not drawn.
    let left = [
        (
            "sidebar-show-symbolic",
            "app.toggle-sidebar",
            "Toggle Sidebar",
        ),
        ("go-up-symbolic", "app.prev-workspace", "Previous Workspace"),
        ("go-down-symbolic", "app.next-workspace", "Next Workspace"),
        (
            "xenon-split-right-symbolic",
            "app.split-right",
            "Split Right",
        ),
        ("xenon-split-down-symbolic", "app.split-down", "Split Down"),
        (
            "sidebar-show-right-symbolic",
            "app.park-pane",
            "Reserve Pane to the Right",
        ),
        ("edit-copy-symbolic", "app.copy", "Copy"),
        ("xenon-copy-clean-symbolic", "app.copy-clean", "Copy Clean"),
        ("edit-paste-symbolic", "app.paste", "Paste"),
    ];
    for (icon, action, label) in left {
        bar.pack_start(&button(icon, action, app.tip(label, action)));
    }
    let right = [
        (
            "utilities-terminal-symbolic",
            "app.new-terminal",
            "New Terminal",
        ),
        (
            "system-search-symbolic",
            "app.open-finder",
            "Go to File\u{2026}",
        ),
        (
            "document-edit-symbolic",
            "app.capture-worklist",
            "Add to Worklist",
        ),
        ("view-list-symbolic", "app.open-worklist", "Open Worklist"),
        (
            "media-playback-start-symbolic",
            "app.run-task",
            "Run Task\u{2026}",
        ),
        ("emblem-system-symbolic", "app.settings", "Settings"),
    ];
    for (icon, action, label) in right.into_iter().rev() {
        bar.pack_end(&button(icon, action, app.tip(label, action)));
    }
    bar
}

pub(crate) fn build(gtk_app: &adw::Application, user_keys: &[xenon_keymap::Entry]) -> Rc<App> {
    // Symbolic icons that exist in every install, whatever the desktop's theme.
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_gtk_icon_theme_name(Some("Adwaita"));
    }
    register_icons();
    let settings: AppSettings = xenon_store::load_settings().unwrap_or_default();
    let probe = PangoProbe(gtk::Label::new(None).pango_context());
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
    let (git, git_rx) = crate::git::GitHub::new();
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
        diffs: RefCell::default(),
        git,
        pulse_source: RefCell::new(None),
        pulse_level: Cell::new(-1),
        started: Instant::now(),
        tab_sync: Cell::new(false),
        refused_drag: Cell::new(false),
        pending_drop: Cell::new(None),
        save_pending: Cell::new(false),
        remote: crate::remote::Remote::new(),
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
        if let Some(app) = weak.upgrade() {
            app.apply_theme();
        }
    });

    crate::actions::install(&app, user_keys);
    app.stack
        .add_named(&crate::empty::all_clear(&app, false), Some("none"));
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
    win.set_content(Some(&layered(&app, &view)));
    {
        let weak = Rc::downgrade(&app);
        app.launcher.set_on_change(move || {
            if let Some(app) = weak.upgrade() {
                app.publish_palette();
            }
        });
    }

    crate::git::listen(&app, git_rx);
    connect_window(&app, &win);
    app.sidebar.set_hints(&app);
    app.sidebar.rebuild(&app.model.borrow());
    start_workspace(&app);
    app.remote_boot();
    app
}

/// The window content with the folderOpen card above it.
fn layered(app: &App, view: &adw::ToolbarView) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(view));
    overlay.add_overlay(&app.trust_card.root);
    overlay
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
        if w.is_active() {
            app.git_refresh_active();
        }
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
