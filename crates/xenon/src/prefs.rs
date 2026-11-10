use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::app::App;
use crate::{prefs_agents, prefs_pages};

const PAGES: [&str; 4] = ["fonts", "editor", "agents", "phone"];

#[derive(Clone)]
pub(crate) struct Shell {
    pub(crate) window: adw::Window,
    toasts: adw::ToastOverlay,
    nav: adw::NavigationView,
    stack: adw::ViewStack,
}

impl Shell {
    pub(crate) fn add_toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }
}

thread_local! {
    static OPEN: RefCell<Option<Shell>> = const { RefCell::new(None) };
}

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
    pub(crate) fn front_window(&self) -> gtk::Window {
        match crate::prefs::open_dialog() {
            Some(shell) if shell.window.is_active() => shell.window.clone().upcast(),
            _ => self.win.clone().upcast(),
        }
    }

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
        add_page(&shell.stack, &prefs_pages::appearance(self));
        add_page(&shell.stack, &prefs_pages::fonts(self));
        add_page(&shell.stack, &prefs_pages::editor(self));
        let agents = prefs_agents::agents(&shell);
        agents.add(&prefs_pages::terminal(self));
        add_page(&shell.stack, &agents);
        add_page(&shell.stack, &crate::prefs_phone::page(self));
        build_layout(&shell);
        install_keys(&shell);
        let before = gtk::prelude::RootExt::focus(&self.win).map(|w| w.downgrade());
        let weak = Rc::downgrade(self);
        window.connect_close_request(move |_| {
            OPEN.with(|o| *o.borrow_mut() = None);
            let Some(app) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            *app.remote.on_change.borrow_mut() = None;
            match before.as_ref().and_then(|w| w.upgrade()) {
                Some(widget) if widget.is_mapped() => {
                    glib::idle_add_local_once(move || {
                        widget.grab_focus();
                    });
                }
                _ => {
                    glib::idle_add_local_once(move || app.focus_terminal());
                }
            }
            glib::Propagation::Proceed
        });
        OPEN.with(|o| *o.borrow_mut() = Some(shell));
        window.present();
    }
}

fn build_layout(shell: &Shell) {
    let list = gtk::ListBox::builder()
        .css_classes(["navigation-sidebar"])
        .build();
    for name in PAGES {
        let Some(page) = shell
            .stack
            .child_by_name(name)
            .map(|c| shell.stack.page(&c))
        else {
            continue;
        };
        let row = gtk::Box::builder().spacing(12).build();
        row.append(&gtk::Image::from_icon_name(
            &page.icon_name().unwrap_or_default(),
        ));
        row.append(&gtk::Label::new(page.title().as_deref()));
        list.append(&row);
    }
    let stack = shell.stack.clone();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let (stack, name) = (stack.clone(), PAGES[row.index() as usize]);
            glib::idle_add_local_once(move || stack.set_visible_child_name(name));
        }
    });
    let content_page = adw::NavigationPage::new(&toolbar(&shell.stack), "Settings");
    let follow = {
        let (list, content_page) = (list.clone(), content_page.clone());
        move |stack: &adw::ViewStack| {
            let Some(page) = stack.visible_child().map(|c| stack.page(&c)) else {
                return;
            };
            content_page.set_title(&page.title().unwrap_or_default());
            let name = stack.visible_child_name().unwrap_or_default();
            if let Some(i) = PAGES.iter().position(|n| *n == name) {
                list.select_row(list.row_at_index(i as i32).as_ref());
            }
        }
    };
    follow(&shell.stack);
    shell.stack.connect_visible_child_notify(follow);
    let side = gtk::ScrolledWindow::builder().child(&list).build();
    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&adw::NavigationPage::new(&toolbar(&side), "Settings")));
    split.set_content(Some(&content_page));
    shell.nav.add(&adw::NavigationPage::new(&split, "Settings"));
    shell.toasts.set_child(Some(&shell.nav));
    shell.window.set_content(Some(&shell.toasts));
}

fn toolbar(content: &impl IsA<gtk::Widget>) -> adw::ToolbarView {
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(content));
    view
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
