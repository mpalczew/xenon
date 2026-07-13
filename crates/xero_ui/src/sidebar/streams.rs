//! Stream rows under a workspace in the sidebar.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;
use xero_core::StreamId;

use super::{DragStream, ROW_H, drag_chip, id_hash};
use crate::app::XeroApp;
use crate::chrome::{self, list_selection};

struct StreamRow<'a> {
    id: StreamId,
    name: &'a str,
    is_active: bool,
}

impl XeroApp {
    pub(super) fn stream_group(
        &self,
        streams: Vec<(StreamId, String, bool)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        // Stream rows only next to the rail; end-drop lives outside so it does
        // not stretch the vertical separator past the titles.
        let rows: Vec<_> = streams
            .iter()
            .map(|(id, name, is_active)| {
                self.stream_row(
                    StreamRow {
                        id: *id,
                        name,
                        is_active: *is_active,
                    },
                    cx,
                )
                .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .pl(px(18.))
            .pr_1()
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .child(
                        div()
                            .w(px(1.))
                            .flex_none()
                            .my_1()
                            .bg(colors.border)
                            .rounded_full(),
                    )
                    .child(div().flex_1().min_w_0().children(rows)),
            )
            .child(self.stream_list_end_drop(cx))
    }

    fn stream_list_end_drop(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let drop_line = colors.drop_target_border;
        div()
            .id("stream-drop-end")
            .h(px(10.))
            .border_t_2()
            .border_color(gpui::transparent_black())
            .can_drop(|drag, _, _| drag.downcast_ref::<DragStream>().is_some())
            .drag_over::<DragStream>(move |style, _, _, _| style.border_color(drop_line))
            .on_drop(cx.listener(|this, dragged: &DragStream, _window, cx| {
                this.reorder_stream(dragged.0, None, cx);
            }))
    }

    fn stream_row(&self, row: StreamRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let StreamRow {
            id,
            name,
            is_active,
        } = row;
        let colors = cx.theme().colors().clone();
        let paint = list_selection(&colors, is_active);
        let group = format!("stream-{id}");
        if let Some(field) = self.rename_field(id) {
            return div()
                .flex()
                .items_center()
                .h(px(ROW_H))
                .px_1()
                .rounded_sm()
                .bg(paint.background)
                .border_l_2()
                .border_color(paint.accent)
                .child(field)
                .into_any_element();
        }
        let attention = self.needs_attention(id).then(|| {
            div()
                .w(px(6.))
                .h(px(6.))
                .rounded_full()
                .bg(chrome::attention_color(cx))
        });
        div()
            .id(("stream", id_hash(id.to_string())))
            .group(group.clone())
            .flex()
            .items_center()
            .justify_between()
            .h(px(ROW_H))
            .px_1()
            .rounded_sm()
            .text_sm()
            .font_weight(if is_active {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .text_color(paint.foreground)
            .bg(paint.background)
            .border_l_2()
            .border_color(paint.accent)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        this.start_rename(id, cx);
                    } else {
                        this.select_stream(id, window, cx);
                    }
                }),
            )
            .on_drag(DragStream(id), drag_chip(name))
            // Insert line, not a full selected fill (avoids “other stream selected”).
            .border_t_2()
            .border_color(gpui::transparent_black())
            .can_drop(move |drag, _, _| {
                drag.downcast_ref::<DragStream>()
                    .is_some_and(|d| d.0 != id)
            })
            .drag_over::<DragStream>(move |style, _, _, _| {
                style.border_color(colors.drop_target_border)
            })
            .on_drop(cx.listener(move |this, dragged: &DragStream, _window, cx| {
                this.reorder_stream(dragged.0, Some(id), cx)
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .pl_1()
                    .child(name.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .children(attention)
                    .child(div().invisible().group_hover(group, |s| s.visible()).child(
                        self.icon_button(
                            ("stream-close", id_hash(id.to_string())),
                            Icon::X,
                            colors.clone(),
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_stream(id, window, cx);
                            }),
                        ),
                    )),
            )
            .into_any_element()
    }
}
