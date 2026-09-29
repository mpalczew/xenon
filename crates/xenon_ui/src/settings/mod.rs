//! Settings window: a page sidebar with search, and one page of rows at a time.
//! Appearance, Fonts, and Editor lead with a live preview of the shell.

mod agents;
mod appearance;
mod controls;
mod dropdowns;
mod field_edit;
mod keys;
mod nav;
mod page;
mod pages;
mod pane;
mod preview;
mod remote_page;
mod row;
mod row_view;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div, point, px,
};
use theme::ActiveTheme;
use xenon_design_system::{
    FocusOnOpen, TextInputAppearance, TextInputConfig, TextInputEvent, TextInputView,
};

use crate::ToggleSettings;
use crate::dropdown::DropdownId;
use field_edit::FieldEdit;
pub(crate) use page::SettingsPage;
use row::visible_rows;

pub struct SettingsView {
    focus: FocusHandle,
    focus_on_open: FocusOnOpen,
    page: SettingsPage,
    /// Keyboard row among `visible_rows` (the page, or search results).
    row: usize,
    /// Paint the row focus edge only once the keyboard is driving.
    keyboard: bool,
    query: String,
    search: Entity<TextInputView>,
    /// Pane child index for each keyboard row, refreshed every paint.
    row_children: Vec<usize>,
    open: Option<DropdownId>,
    filter: String,
    filter_input: Entity<TextInputView>,
    highlight: usize,
    field_edit: Option<FieldEdit>,
    /// This window's toast host, created on first render (it needs the window).
    toast: Option<Entity<xenon_design_system::ToastView>>,
    scroll: gpui::ScrollHandle,
    _subscriptions: [Subscription; 2],
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter_input = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line("Type to filter…").parent_navigation(),
                cx,
            )
        });
        let search = cx.new(|cx| {
            TextInputView::new(
                TextInputConfig::single_line("Search settings")
                    .appearance(TextInputAppearance::Inline)
                    .min_height(px(18.))
                    .parent_navigation(),
                cx,
            )
        });
        let filter_sub = cx.subscribe_in(&filter_input, window, Self::on_filter_event);
        let search_sub = cx.subscribe_in(&search, window, Self::on_search_event);
        // Full system font scans are deferred and never run on paint / key path.
        cx.spawn(async move |this, cx| {
            this.update(cx, |_this, cx| {
                crate::dropdown::warm_mono_font_families(cx);
                crate::dropdown::warm_ui_font_families(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        let focus = cx.focus_handle();
        let mut focus_on_open = FocusOnOpen::new(focus.clone());
        focus_on_open.open();
        Self {
            focus,
            focus_on_open,
            page: SettingsPage::Appearance,
            row: 0,
            keyboard: false,
            query: String::new(),
            search,
            row_children: Vec::new(),
            open: None,
            filter: String::new(),
            filter_input,
            highlight: 0,
            field_edit: None,
            toast: None,
            scroll: gpui::ScrollHandle::new(),
            _subscriptions: [filter_sub, search_sub],
        }
    }

    pub(crate) fn show_page(
        &mut self,
        page: SettingsPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal(page, 0, window, cx);
    }

    /// Leave search and put the keyboard on one row of a page.
    fn reveal(
        &mut self,
        page: SettingsPage,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_query(String::new(), cx);
        self.dismiss_dropdown(window, cx);
        if self.page != page {
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.page = page;
        self.row = row;
        self.focus.focus(window, cx);
        self.scroll_to_row();
        cx.notify();
    }

    fn focus_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.row = row;
        self.keyboard = false;
        if self.field_edit.is_none() && self.open.is_none() {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn scroll_to_row(&self) {
        if let Some(&child) = self.row_children.get(self.row) {
            self.scroll.scroll_to_item(child);
        }
    }

    fn focus_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| input.open(cx));
    }

    fn start_search(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |input, cx| input.set_text(typed, cx));
        self.set_query(typed.to_string(), cx);
        self.focus_search(window, cx);
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_query(String::new(), cx);
        self.focus.focus(window, cx);
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if query.is_empty() {
            self.search.update(cx, |input, cx| input.set_text("", cx));
        }
        if self.query != query {
            self.query = query;
            self.row = 0;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        cx.notify();
    }

    fn on_search_event(
        &mut self,
        _: &Entity<TextInputView>,
        event: &TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextInputEvent::Changed(query) => self.set_query(query.clone(), cx),
            TextInputEvent::ParentKey { key, .. } => match key.as_str() {
                "enter" => {
                    let hit = visible_rows(self.page, &self.query, cx)
                        .into_iter()
                        .nth(self.row);
                    if let Some(hit) = hit {
                        self.reveal(hit.page, hit.index, window, cx);
                    }
                }
                "tab" => self.focus.focus(window, cx),
                "escape" => self.clear_search(window, cx),
                key => {
                    self.keyboard |= self.navigate(key, window, cx);
                }
            },
            TextInputEvent::Submit(_) | TextInputEvent::Cancel => {}
        }
    }
}

#[cfg(feature = "visual-tests")]
impl SettingsView {
    /// Visual tests: open a page, or search results for `query`.
    pub fn visual_show(
        &mut self,
        page: SettingsPage,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_page(page, window, cx);
        self.keyboard = !query.is_empty();
        if !query.is_empty() {
            self.search
                .update(cx, |input, cx| input.set_text(query, cx));
            self.set_query(query.to_string(), cx);
        }
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title("Settings");
        self.focus_on_open.focus_after_open(window, cx);
        let ui = xenon_settings::ui_font(cx);
        window.set_rem_size(gpui::px(ui.size));
        let colors = cx.theme().colors().clone();
        let selected = self.query.is_empty().then_some(self.page);
        let sidebar = nav::sidebar(selected, &self.search, remote_page::is_on(cx), cx);
        let pane = self.pane(window, cx);
        let toast = self
            .toast
            .get_or_insert_with(|| xenon_design_system::toast_host(gpui::px(0.), window, cx))
            .clone();
        div()
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_action(cx.listener(|_, _: &ToggleSettings, window, _cx| {
                window.remove_window();
            }))
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .flex()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .font_family(ui.family)
            .child(sidebar)
            .child(pane)
            .child(toast)
    }
}
