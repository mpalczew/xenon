//! One pane: an `AdwTabBar` above an `AdwTabView` that holds the tab bodies.
//! The view mirrors the model; what the user does on the bar is reported to
//! the model (`tab_ui.rs`), which decides, and the next `sync` draws the answer.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use xenon_core::{PaneId, PaneLeaf, TabId, WorkspaceDot, WorkspaceId};

use crate::app::App;
use crate::model::Tab;

pub(crate) struct PaneView {
    pub root: gtk::Box,
    bar: adw::TabBar,
    view: adw::TabView,
    /// The workspace has more than one pane (every pane then shows its bar).
    multi: Cell<bool>,
    /// The tab view, or the empty card when the pane has no tabs.
    stack: gtk::Stack,
    pages: RefCell<HashMap<TabId, adw::TabPage>>,
    ws: WorkspaceId,
    pane: PaneId,
    /// "Remove empty pane" on the empty card; shown only with company.
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
            if let Some(app) = weak.upgrade() {
                app.new_terminal_in(ws, pane);
            }
        });
        let view = adw::TabView::new();
        // Only the shortcuts our key map binds (Ctrl+Tab, Ctrl+Shift+Tab);
        // the others would take keys from the terminal.
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
            bar,
            view,
            multi: Cell::new(false),
            stack,
            pages: RefCell::default(),
            ws,
            pane,
            remove: RefCell::default(),
        });
        this.connect(app);
        crate::pane_drop::install(app, &body, ws, pane);
        this
    }

    /// Report what the user does on the bar. Our own changes in `sync` are
    /// silent (`App::tab_sync`).
    fn connect(&self, app: &Rc<App>) {
        let (ws, pane) = (self.ws, self.pane);
        // A tab dropped outside every bar and pane is refused: Xenon is one
        // window. The bar insists on a view to put it in, so it goes back into
        // its own, and the next `sync` puts it in its place.
        let weak = Rc::downgrade(app);
        self.view.connect_create_window(move |view| {
            if let Some(app) = weak.upgrade() {
                app.refused_drag.set(true);
            }
            Some(view.clone())
        });
        let weak = Rc::downgrade(app);
        self.view.connect_close_page(move |view, page| {
            // The page stays until the model's answer removes it, so the
            // close button does exactly what the close-tab action does.
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
        // A drag between bars (or onto a pane body) is over once the page is
        // back in a view; only then does the model hear about it.
        let weak = Rc::downgrade(app);
        self.view.connect_is_transferring_page_notify(move |view| {
            if view.is_transferring_page() {
                return;
            }
            if let Some(app) = weak.upgrade().filter(|a| !a.tab_sync.get()) {
                gtk::glib::idle_add_local_once(move || app.tab_transfer_done(ws));
            }
        });
    }

    /// The bodies shown in this view, in tab order.
    pub(crate) fn order(&self) -> Vec<gtk::Widget> {
        (0..self.view.n_pages())
            .map(|at| self.view.nth_page(at).child())
            .collect()
    }

    /// Make the pane show exactly `leaf`'s tabs. `bodies` holds each tab's
    /// widget; a body that sits in another pane's view is moved here.
    pub(crate) fn sync(
        &self,
        app: &Rc<App>,
        leaf: &PaneLeaf<Tab>,
        bodies: &HashMap<TabId, gtk::Widget>,
    ) {
        // A lone tab in a lone pane needs no bar; the header names it.
        self.bar
            .set_visible(self.multi.get() || leaf.tabs.len() > 1);
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

    /// The page showing `body` in this view, taking it from wherever it was.
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
                if let Some(app) = weak.upgrade() {
                    app.remove_empty_pane(ws, pane);
                }
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

    /// Show a tab's status on its page. Selected tabs carry no status.
    pub(crate) fn paint(&self, tab: &Tab, index: usize, selected: bool, dot: Option<WorkspaceDot>) {
        let pages = self.pages.borrow();
        let Some(page) = pages.get(&tab.id) else {
            return;
        };
        let dot = if selected { None } else { dot };
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

/// True when the keyboard focus of `widget`'s window is `widget` or inside it.
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
