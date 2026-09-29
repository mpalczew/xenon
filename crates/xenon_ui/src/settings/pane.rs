//! The right-hand pane: preview (where it applies), page title, and rows.
//! Rows are direct children of the scroll container so keyboard moves can
//! scroll a row into view.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};

use super::SettingsView;
use super::page::SettingsPage;
use super::preview::live_preview;
use super::row::{Group, page_groups};
use super::row_view::{PaintState, Placement, setting_row};

impl SettingsView {
    pub(super) fn pane(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let groups = if self.query.is_empty() {
            page_groups(self.page, cx)
        } else {
            search_groups(&self.query, cx)
        };
        let state = PaintState {
            open: self.open,
            filter: &self.filter,
            filter_input: &self.filter_input,
            highlight: self.highlight,
            viewport_height: window.viewport_size().height,
            editing: self
                .field_edit
                .as_ref()
                .map(|edit| (edit.field, edit.input.clone())),
        };
        let mut children: Vec<AnyElement> = Vec::new();
        if self.query.is_empty() && self.page.has_preview() {
            children.push(div().pb_4().child(live_preview(cx)).into_any_element());
        }
        children.push(header(self.page, &self.query, cx).into_any_element());
        let mut row_children = Vec::new();
        let mut index = 0;
        for group in groups {
            if let Some(title) = group.title {
                children.push(group_title(title, cx).into_any_element());
            }
            let count = group.rows.len();
            for (position, row) in group.rows.into_iter().enumerate() {
                let at = Placement {
                    index,
                    focused: self.keyboard && index == self.row,
                    first: position == 0,
                    last: position + 1 == count,
                };
                row_children.push(children.len());
                children.push(setting_row(row, at, &state, cx));
                index += 1;
            }
            children.push(div().h(px(16.)).into_any_element());
        }
        if index == 0 {
            children.push(no_results(cx).into_any_element());
        }
        self.row_children = row_children;
        div()
            .id("settings-pane")
            .track_scroll(&self.scroll)
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .px_5()
            .pt_4()
            .pb_5()
            .on_click(cx.listener(|this, _, window, cx| this.dismiss_dropdown(window, cx)))
            .children(children)
            .into_any_element()
    }
}

/// Every matching row, grouped under its page title.
fn search_groups(query: &str, cx: &gpui::App) -> Vec<Group> {
    SettingsPage::ALL
        .iter()
        .filter_map(|&page| {
            let rows: Vec<_> = page_groups(page, cx)
                .into_iter()
                .flat_map(|group| group.rows)
                .filter(|row| row.matches(query))
                .collect();
            (!rows.is_empty()).then(|| Group {
                title: Some(page.title().into()),
                rows,
            })
        })
        .collect()
}

fn header(
    page: SettingsPage,
    query: &str,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let (title, sub): (SharedString, Option<SharedString>) = if query.is_empty() {
        (page.title().into(), None)
    } else {
        ("Results".into(), Some(format!("for “{query}”").into()))
    };
    div()
        .flex()
        .items_baseline()
        .gap_2()
        .pb_3()
        .child(div().type_role(TypeRole::SectionTitle, cx).child(title))
        .children(sub.map(|sub| {
            div()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(colors.text_muted)
                .child(sub)
        }))
}

fn group_title(title: SharedString, cx: &mut Context<SettingsView>) -> impl IntoElement + use<> {
    div()
        .pb_1()
        .pl(px(2.))
        .type_role(TypeRole::ControlLabel, cx)
        .child(title)
}

fn no_results(cx: &mut Context<SettingsView>) -> impl IntoElement + use<> {
    div()
        .type_role(TypeRole::Supporting, cx)
        .child("No settings match. Escape clears the search.")
}
