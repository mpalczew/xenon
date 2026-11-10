use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_core::{PaneId, PaneLeaf, TabId, WorkspaceDot, WorkspaceId};

use crate::app::{App, defer};
use crate::model::Tab;

pub(crate) struct PaneView {
    pub root: gtk::Box,
    view: adw::TabView,
    multi: Cell<bool>,
    stack: gtk::Stack,
    pages: RefCell<HashMap<TabId, adw::TabPage>>,
    ws: WorkspaceId,
    pane: PaneId,
    remove: RefCell<Option<gtk::Button>>,
}

impl PaneView {
    pub(crate) fn new(app: &Rc<App>, ws: WorkspaceId, pane: PaneId) -> Rc<Self> {
        let add = gtk::Button::from_icon_name("list-add-symbolic");
        add.add_css_class("flat");
        add.set_tooltip_text(Some(&app.tip("New Terminal", "app.new-terminal")));
        add.set_focusable(false);
        let weak = Rc::downgrade(app);
        add.connect_clicked(move |_| {
            defer(&weak, move |app| app.new_terminal_in(ws, pane));
        });
        let view = adw::TabView::new();
        view.set_shortcuts(
            adw::TabViewShortcuts::CONTROL_TAB | adw::TabViewShortcuts::CONTROL_SHIFT_TAB,
        );
        view.set_vexpand(true);
        view.set_hexpand(true);
        let bar = adw::TabBar::new();
        bar.set_view(Some(&view));
        bar.set_autohide(false);
        bar.set_expand_tabs(false);
        bar.set_end_action_widget(Some(&add));
        bar.add_css_class("inline");
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);
        stack.add_named(&view, Some("tabs"));
        let body = gtk::Overlay::new();
        body.set_child(Some(&stack));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("xpane");
        root.append(&bar);
        root.append(&body);
        let this = Rc::new(Self {
            root,
            view,
            multi: Cell::new(false),
            stack,
            pages: RefCell::default(),
            ws,
            pane,
            remove: RefCell::default(),
        });
        this.connect(app);
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(app);
        focus.connect_enter(move |_| {
            defer(&weak, move |app| app.pane_focused(ws, pane));
        });
        this.root.add_controller(focus);
        // A click in a pane makes it the focused pane even when the clicked
        // view takes no keyboard focus itself (the worklist's task list).
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (weak, inner) = (Rc::downgrade(app), body.clone());
        click.connect_pressed(move |_, _, _, _| {
            let inner = inner.clone();
            defer(&weak, move |app| {
                if !focus_inside(&inner) {
                    inner.child_focus(gtk::DirectionType::TabForward);
                }
                app.pane_focused(ws, pane);
            });
        });
        body.add_controller(click);
        crate::pane_drop::install(app, &body, ws, pane);
        this
    }

    fn connect(&self, app: &Rc<App>) {
        let (ws, pane) = (self.ws, self.pane);
        // Xenon is one window. A drag that ends off every bar (a pane body
        // drop included) makes the bar ask for a new window and attach the
        // page there at position 0. We hand back this view; the sync at the
        // end of the drag puts the page where the model says.
        let weak = Rc::downgrade(app);
        self.view.connect_create_window(move |view| {
            if let Some(app) = weak.upgrade() {
                app.off_bar.set(true);
            }
            Some(view.clone())
        });
        let weak = Rc::downgrade(app);
        // sync: answer or ordering before return: `close_page_finish` is the answer to the signal and must be given before it returns.
        self.view.connect_close_page(move |view, page| {
            let app = weak.upgrade().filter(|a| !a.tab_sync.get());
            view.close_page_finish(page, app.is_none());
            if let Some(app) = app {
                app.tab_event(ws, page.child(), |app, ws, tab| app.close_tab_ui(ws, tab));
            }
            gtk::glib::Propagation::Stop
        });
        let weak = Rc::downgrade(app);
        self.view.connect_selected_page_notify(move |view| {
            let Some(app) = weak.upgrade().filter(|a| !a.tab_sync.get()) else {
                return;
            };
            if view.is_transferring_page() {
                return;
            }
            if let Some(page) = view.selected_page() {
                app.tab_event(ws, page.child(), move |app, ws, tab| {
                    app.tab_selected(ws, pane, tab);
                });
            }
        });
        let weak = Rc::downgrade(app);
        self.view.connect_page_reordered(move |view, page, at| {
            if view.is_transferring_page() {
                return;
            }
            if let Some(app) = weak.upgrade().filter(|a| !a.tab_sync.get()) {
                app.tab_event(ws, page.child(), move |app, ws, tab| {
                    app.tab_placed(ws, pane, tab, at.max(0) as usize);
                });
            }
        });
        let weak = Rc::downgrade(app);
        self.view.connect_is_transferring_page_notify(move |view| {
            if view.is_transferring_page() {
                return;
            }
            if let Some(app) = weak.upgrade().filter(|a| !a.tab_sync.get())
                && !app.transfer_queued.replace(true)
            {
                gtk::glib::idle_add_local_once(move || app.tab_transfer_done(ws));
            }
        });
    }

    pub(crate) fn order(&self) -> Vec<gtk::Widget> {
        (0..self.view.n_pages())
            .map(|at| self.view.nth_page(at).child())
            .collect()
    }

    pub(crate) fn sync(
        &self,
        app: &Rc<App>,
        leaf: &PaneLeaf<Tab>,
        bodies: &HashMap<TabId, gtk::Widget>,
    ) {
        // A page whose tab is gone is closed. One whose tab lives in another
        // pane now stays until that pane takes it with `transfer_page`: a
        // closed page keeps its body inside the page's own wrapper, so the
        // body could not be added to another view afterwards.
        for at in (0..self.view.n_pages()).rev() {
            let page = self.view.nth_page(at);
            if !bodies.values().any(|body| *body == page.child()) {
                self.view.close_page(&page);
            }
        }
        let mut pages = self.pages.borrow_mut();
        pages.clear();
        let mut at = 0;
        for tab in &leaf.tabs {
            let Some(body) = bodies.get(&tab.id) else {
                continue;
            };
            let Some(page) = self.page_for(body, at) else {
                continue;
            };
            if self.view.page_position(&page) != at {
                self.view.reorder_page(&page, at);
            }
            pages.insert(tab.id, page);
            at += 1;
        }
        match leaf.active_tab().and_then(|t| pages.get(&t.id)) {
            Some(page) => {
                if self.view.selected_page().as_ref() != Some(page) {
                    self.view.set_selected_page(page);
                }
                self.stack.set_visible_child_name("tabs");
            }
            None => self.show_empty(app),
        }
    }

    fn page_for(&self, body: &gtk::Widget, at: i32) -> Option<adw::TabPage> {
        let home = body
            .ancestor(adw::TabView::static_type())
            .and_then(|w| w.downcast::<adw::TabView>().ok());
        match home {
            Some(view) if view == self.view => Some(view.page(body)),
            Some(other) => {
                let page = other.page(body);
                other.transfer_page(&page, &self.view, at);
                Some(page)
            }
            // A page the bar holds mid-drag has a wrapper but no view: it
            // comes back when the drag ends, and the next `sync` takes it.
            None if body.parent().is_some() => None,
            None => Some(self.view.insert(body, at)),
        }
    }

    fn show_empty(&self, app: &Rc<App>) {
        if self.stack.child_by_name("empty").is_none() {
            let card = crate::empty::all_clear(app, true);
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&card));
            let click = gtk::GestureClick::new();
            click.connect_pressed(glib::clone!(
                #[weak]
                overlay,
                move |_, _, _, _| {
                    glib::idle_add_local_once(move || {
                        if !focus_inside(&overlay) {
                            overlay.child_focus(gtk::DirectionType::TabForward);
                        }
                    });
                }
            ));
            overlay.add_controller(click);
            let remove = gtk::Button::from_icon_name("window-close-symbolic");
            remove.add_css_class("flat");
            remove.add_css_class("xremove-pane");
            remove.set_halign(gtk::Align::End);
            remove.set_valign(gtk::Align::Start);
            remove.set_margin_top(8);
            remove.set_margin_end(8);
            remove.set_focusable(false);
            remove.set_tooltip_text(Some("Remove empty pane"));
            let weak = Rc::downgrade(app);
            let (ws, pane) = (self.ws, self.pane);
            remove.connect_clicked(move |_| {
                defer(&weak, move |app| {
                    app.remove_empty_pane(ws, pane);
                });
            });
            self.remove.replace(Some(remove.clone()));
            overlay.add_overlay(&remove);
            self.stack.add_named(&overlay, Some("empty"));
        }
        if let Some(remove) = self.remove.borrow().as_ref() {
            remove.set_visible(self.multi.get());
        }
        self.stack.set_visible_child_name("empty");
    }

    pub(crate) fn focus_empty(&self) {
        if let Some(card) = self.stack.child_by_name("empty") {
            card.child_focus(gtk::DirectionType::TabForward);
        }
    }

    pub(crate) fn paint(
        &self,
        tab: &Tab,
        index: usize,
        dot: Option<WorkspaceDot>,
        path: Option<String>,
    ) {
        let pages = self.pages.borrow();
        let Some(page) = pages.get(&tab.id) else {
            return;
        };
        let label = tab.label(index);
        let title = if tab.title.is_empty() {
            label.clone()
        } else {
            tab.title.clone()
        };
        page.set_title(&label);
        page.set_needs_attention(matches!(dot, Some(WorkspaceDot::Attention(_))));
        let icon = if tab.exited.is_some() {
            Some("process-stop-symbolic")
        } else if matches!(dot, Some(WorkspaceDot::Working)) {
            Some("media-record-symbolic")
        } else {
            None
        };
        page.set_indicator_icon(icon.map(gio::ThemedIcon::new).as_ref());
        let tip = if tab.exited.is_some() {
            format!("{title} \u{2014} process exited")
        } else if let Some(dot) = dot {
            format!("{title} \u{2014} {}", dot.tooltip())
        } else {
            title
        };
        let tip = match path {
            Some(path) => format!("{tip}\n{path}"),
            None => tip,
        };
        page.set_tooltip(&tip);
    }

    pub(crate) fn set_title(&self, tab: TabId, text: &str) {
        if let Some(page) = self.pages.borrow().get(&tab) {
            page.set_title(text);
        }
    }

    pub(crate) fn set_multi(&self, multi: bool) {
        self.multi.set(multi);
    }
}

pub(crate) fn focus_inside(widget: &impl IsA<gtk::Widget>) -> bool {
    let widget = widget.as_ref();
    widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| focus == *widget || focus.is_ancestor(widget))
}

pub(crate) fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}
