//! The Settings window (Ctrl+,): its own top-level window, transient for the
//! main one, with a sidebar of pages (`AdwViewSwitcherSidebar`) beside the
//! `AdwPreferencesPage`s. Ctrl+1 to Ctrl+5 jump to a page, Escape (or Ctrl+W)
//! closes, or backs out of the theme picker first, and focus returns to where
//! it was. `app.settings` opens it, or raises it when it is already open.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::app::App;
use crate::{prefs_agents, prefs_pages, prefs_picker, prefs_theme};

const PAGES: [&str; 5] = ["appearance", "fonts", "editor", "agents", "phone"];

/// The open Settings window and the parts the pages talk to.
#[derive(Clone)]
pub(crate) struct Shell {
    window: adw::Window,
    toasts: adw::ToastOverlay,
    nav: adw::NavigationView,
    stack: adw::ViewStack,
}

impl Shell {
    pub(crate) fn add_toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// Show a sub page (the theme picker) over the page list.
    pub(crate) fn push(&self, page: &adw::NavigationPage) {
        self.nav.push(page);
    }

    pub(crate) fn pop(&self) {
        self.nav.pop();
    }

    pub(crate) fn close(&self) {
        self.window.close();
    }

    pub(crate) fn page_name(&self) -> Option<glib::GString> {
        self.stack.visible_child_name()
    }

    pub(crate) fn show_page(&self, name: &str) {
        self.stack.set_visible_child_name(name);
    }
}

thread_local! {
    static OPEN: RefCell<Option<Shell>> = const { RefCell::new(None) };
}

/// The Settings window while it is open.
pub(crate) fn open_dialog() -> Option<Shell> {
    OPEN.with(|o| o.borrow().clone())
}

fn add_page(stack: &adw::ViewStack, page: &adw::PreferencesPage) {
    let title = page.title();
    let icon = page.icon_name().unwrap_or_default();
    let name = page.name().unwrap_or_default();
    stack.add_titled_with_icon(page, Some(&name), &title, &icon);
}

impl App {
    /// Open Settings, or raise it when it is already open.
    pub(crate) fn toggle_settings(self: &Rc<Self>) {
        if let Some(shell) = open_dialog() {
            shell.window.present();
            return;
        }
        self.launcher.close();
        let window = adw::Window::builder()
            .title("Settings")
            .default_width(860)
            .default_height(640)
            .transient_for(&self.win)
            .build();
        let shell = Shell {
            window: window.clone(),
            toasts: adw::ToastOverlay::new(),
            nav: adw::NavigationView::new(),
            stack: adw::ViewStack::new(),
        };
        add_page(&shell.stack, &prefs_theme::page(self, &shell));
        add_page(&shell.stack, &prefs_pages::fonts(self));
        add_page(&shell.stack, &prefs_pages::editor(self));
        let agents = prefs_agents::agents(&shell);
        agents.add(&prefs_pages::terminal(self));
        agents.add(&prefs_agents::servers());
        add_page(&shell.stack, &agents);
        add_page(&shell.stack, &crate::prefs_phone::page(self));
        build_layout(&shell);
        install_keys(&shell);
        // One sheet paints every tile in its own theme's colors.
        let tiles = gtk::CssProvider::new();
        tiles.load_from_string(&prefs_picker::tile_css(&prefs_theme::tile_names(self)));
        let display = gtk::prelude::WidgetExt::display(&self.win);
        gtk::style_context_add_provider_for_display(
            &display,
            &tiles,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let before = gtk::prelude::RootExt::focus(&self.win).map(|w| w.downgrade());
        let weak = Rc::downgrade(self);
        window.connect_close_request(move |_| {
            gtk::style_context_remove_provider_for_display(&display, &tiles);
            OPEN.with(|o| *o.borrow_mut() = None);
            crate::prefs_state::forget_all();
            let Some(app) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            *app.remote.on_change.borrow_mut() = None;
            app.end_preview();
            match before.as_ref().and_then(|w| w.upgrade()) {
                Some(widget) if widget.is_mapped() => {
                    widget.grab_focus();
                }
                _ => app.focus_terminal(),
            }
            // Focus settles on the next idle; publish after it.
            let weak = Rc::downgrade(&app);
            glib::idle_add_local_once(move || {
                if let Some(app) = weak.upgrade() {
                    app.publish_settings();
                }
            });
            glib::Propagation::Proceed
        });
        {
            let weak = Rc::downgrade(self);
            shell.stack.connect_visible_child_name_notify(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.publish_settings();
                }
            });
            let s = shell.clone();
            crate::prefs_state::register("close", move |_| s.close());
        }
        OPEN.with(|o| *o.borrow_mut() = Some(shell));
        window.present();
        self.publish_settings();
    }
}

/// Sidebar of pages on the left, the visible page on the right.
fn build_layout(shell: &Shell) {
    let sidebar = adw::ViewSwitcherSidebar::new();
    sidebar.set_stack(Some(&shell.stack));
    let side_view = adw::ToolbarView::new();
    side_view.add_top_bar(&adw::HeaderBar::new());
    side_view.set_content(Some(&sidebar));
    let content = adw::ToolbarView::new();
    content.add_top_bar(&adw::HeaderBar::new());
    content.set_content(Some(&shell.stack));
    let content_page = adw::NavigationPage::new(&content, "Settings");
    let stack = shell.stack.clone();
    let follow = {
        let content_page = content_page.clone();
        move |stack: &adw::ViewStack| {
            if let Some(page) = stack.visible_child().map(|c| stack.page(&c)) {
                content_page.set_title(&page.title().unwrap_or_default());
            }
        }
    };
    follow(&stack);
    stack.connect_visible_child_notify(follow);
    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&adw::NavigationPage::new(&side_view, "Settings")));
    split.set_content(Some(&content_page));
    shell.nav.add(&adw::NavigationPage::new(&split, "Settings"));
    shell.toasts.set_child(Some(&shell.nav));
    shell.window.set_content(Some(&shell.toasts));
}

fn install_keys(shell: &Shell) {
    let controller = gtk::ShortcutController::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    for (index, name) in PAGES.iter().enumerate() {
        let weak = shell.stack.downgrade();
        let action = gtk::CallbackAction::new(move |_, _| {
            if let Some(stack) = weak.upgrade() {
                stack.set_visible_child_name(name);
            }
            glib::Propagation::Stop
        });
        if let Some(trigger) =
            gtk::ShortcutTrigger::parse_string(&format!("<Control>{}", index + 1))
        {
            controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
        }
    }
    shell.window.add_controller(controller);
    // Escape reaches here only when the picker did not take it.
    let closing = gtk::ShortcutController::new();
    for key in ["Escape", "<Control>w"] {
        let weak = shell.window.downgrade();
        let action = gtk::CallbackAction::new(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
            glib::Propagation::Stop
        });
        if let Some(trigger) = gtk::ShortcutTrigger::parse_string(key) {
            closing.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
        }
    }
    shell.window.add_controller(closing);
}
