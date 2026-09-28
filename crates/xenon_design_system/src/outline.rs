//! Title plus indented points. The caller stores and validates the text.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Render, Styled, Subscription,
    Window, div, px,
};
use theme::ActiveTheme;

use crate::outline_text::{OutlinePoint, details_from_points, points_from_details};
use crate::text_input::{TextInputAppearance, TextInputConfig, TextInputEvent, TextInputView};
use crate::typography::{TypeRole, Typography};

pub enum OutlineEvent {
    Changed,
    Submit,
    Cancel,
    /// ⌘⌫ while a field is focused.
    Delete,
    Checked(bool),
}

struct PointField {
    depth: u8,
    input: Entity<TextInputView>,
    _subscription: Subscription,
}

pub struct OutlineView {
    title: Entity<TextInputView>,
    points: Vec<PointField>,
    title_check: Option<bool>,
    _title_subscription: Subscription,
}

impl EventEmitter<OutlineEvent> for OutlineView {}

impl OutlineView {
    pub fn new(title: &str, details: &str, checked: Option<bool>, cx: &mut Context<Self>) -> Self {
        let title = field("Name the work item", title, cx);
        let _title_subscription = subscribe(&title, cx);
        let points = points_from_details(details)
            .into_iter()
            .map(|point| point_field(&point.text, point.depth, cx))
            .collect();
        Self {
            title,
            points,
            title_check: checked,
            _title_subscription,
        }
    }

    pub fn title(&self, cx: &App) -> String {
        self.title.read(cx).text().to_owned()
    }

    pub fn details(&self, cx: &App) -> String {
        details_from_points(&self.points(cx))
    }

    pub fn set_title_placeholder(&self, placeholder: impl Into<String>, cx: &mut App) {
        self.title
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
    }

    pub fn open_title(&self, cx: &mut App) {
        self.title.update(cx, |input, cx| input.open(cx));
    }

    pub fn replace(&mut self, title: &str, details: &str, cx: &mut Context<Self>) {
        self.title.update(cx, |input, cx| input.set_text(title, cx));
        self.points = points_from_details(details)
            .into_iter()
            .map(|point| point_field(&point.text, point.depth, cx))
            .collect();
        cx.emit(OutlineEvent::Changed);
        cx.notify();
    }

    fn points(&self, cx: &App) -> Vec<OutlinePoint> {
        self.points
            .iter()
            .map(|point| OutlinePoint {
                text: point.input.read(cx).text().to_owned(),
                depth: point.depth,
            })
            .collect()
    }

    fn on_title_key(&mut self, key: &str, cx: &mut Context<Self>) {
        if !matches!(key, "enter" | "down") {
            return;
        }
        if self.points.is_empty() {
            self.points.push(point_field("", 0, cx));
        }
        let first = self.points[0].input.clone();
        first.update(cx, |input, cx| {
            input.place_caret(0, cx);
            input.open(cx);
        });
        cx.notify();
    }

    fn on_point_key(&mut self, index: usize, key: FieldKey<'_>, cx: &mut Context<Self>) {
        let FieldKey { key, shift, source } = key;
        match key {
            "up" if index == 0 => {
                let at = self.title.read(cx).text().chars().count();
                self.title.update(cx, |input, cx| {
                    input.place_caret(at, cx);
                    input.open(cx);
                });
            }
            "up" => self.focus_point(index - 1, usize::MAX, cx),
            "down" if index + 1 < self.points.len() => self.focus_point(index + 1, 0, cx),
            "tab" => {
                if shift {
                    self.points[index].depth = self.points[index].depth.saturating_sub(1);
                } else {
                    self.points[index].depth = self.points[index].depth.saturating_add(1);
                }
            }
            "enter" => {
                let suffix = source.update(cx, |input, cx| input.split_off_suffix(cx));
                let depth = self.points[index].depth;
                self.points
                    .insert(index + 1, point_field(&suffix, depth, cx));
                self.focus_point(index + 1, 0, cx);
            }
            "backspace" => self.backspace_point(index, cx),
            _ => return,
        }
        cx.emit(OutlineEvent::Changed);
        cx.notify();
    }

    fn backspace_point(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.points[index].depth > 0 {
            self.points[index].depth -= 1;
            return;
        }
        let text = self.points[index].input.read(cx).text().to_owned();
        self.points.remove(index);
        if index == 0 {
            let at = self.title.read(cx).text().chars().count();
            self.title.update(cx, |input, cx| {
                let joined = format!("{}{text}", input.text());
                input.set_text(joined, cx);
                input.place_caret(at, cx);
                input.open(cx);
            });
            return;
        }
        let previous = index - 1;
        let at = self.points[previous].input.read(cx).text().chars().count();
        self.points[previous].input.update(cx, |input, cx| {
            let joined = format!("{}{text}", input.text());
            input.set_text(joined, cx);
            input.place_caret(at, cx);
            input.open(cx);
        });
    }

    fn focus_point(&self, index: usize, at: usize, cx: &mut Context<Self>) {
        self.points[index].input.update(cx, |input, cx| {
            input.place_caret(at, cx);
            input.open(cx);
        });
    }

    fn on_field_key(&mut self, key: FieldKey<'_>, platform: bool, cx: &mut Context<Self>) {
        if key.key == "escape" {
            cx.emit(OutlineEvent::Cancel);
            return;
        }
        if key.key == "backspace" && platform {
            cx.emit(OutlineEvent::Delete);
            return;
        }
        if key.key == "enter" && platform {
            cx.emit(OutlineEvent::Submit);
            return;
        }
        if key.source == self.title {
            self.on_title_key(key.key, cx);
            return;
        }
        let Some(index) = self
            .points
            .iter()
            .position(|point| point.input == key.source)
        else {
            return;
        };
        self.on_point_key(index, key, cx);
    }

    fn place_click(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.nearest_field(event.position.y, cx);
        let extend = event.modifiers.shift;
        target.update(cx, |input, cx| {
            input.place_at(event.position, extend, window, cx);
        });
    }

    fn nearest_field(&self, y: Pixels, cx: &App) -> Entity<TextInputView> {
        let mut best = self.title.clone();
        let mut best_distance = px(f32::MAX);
        for input in
            std::iter::once(&self.title).chain(self.points.iter().map(|point| &point.input))
        {
            let distance = input.read(cx).vertical_distance(y);
            if distance < best_distance {
                best_distance = distance;
                best = input.clone();
            }
        }
        best
    }
}

struct FieldKey<'a> {
    key: &'a str,
    shift: bool,
    source: Entity<TextInputView>,
}

impl Focusable for OutlineView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title.read(cx).focus_handle()
    }
}

impl Render for OutlineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let points = self
            .points
            .iter()
            .map(|point| (point.depth, point.input.clone()))
            .collect::<Vec<_>>();
        div()
            .id("outline-body")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.place_click(event, window, cx);
                }),
            )
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .child(title_line(
                self.title_check,
                self.title.clone(),
                !self.points.is_empty(),
                cx,
            ))
            .children({
                let mut rows = Vec::new();
                for (depth, input) in points {
                    rows.push(point_line(depth, input, cx).into_any_element());
                }
                rows
            })
    }
}

fn title_line(
    checked: Option<bool>,
    input: Entity<TextInputView>,
    spaced: bool,
    cx: &mut Context<OutlineView>,
) -> impl IntoElement {
    let row = div().flex().items_center().gap_2().h(px(28.));
    let row = if spaced { row.mb_3() } else { row };
    row.type_role(TypeRole::ListPrimary, cx)
        .children(checked.map(|checked| {
            div()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(crate::checkbox(
                    "outline-title-check",
                    crate::CheckboxState {
                        checked,
                        disabled: false,
                    },
                    "Complete item",
                    cx,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.title_check = Some(!checked);
                        cx.emit(OutlineEvent::Checked(!checked));
                        cx.notify();
                    }),
                ))
        }))
        .child(div().flex_1().min_w_0().child(input))
}

fn point_line(
    depth: u8,
    input: Entity<TextInputView>,
    cx: &mut Context<OutlineView>,
) -> impl IntoElement {
    let color = cx.theme().colors().text_accent;
    let size = if depth == 0 { px(5.) } else { px(4.) };
    div()
        .flex()
        .items_center()
        .gap_2()
        .h(px(28.))
        .pl(px(f32::from(depth) * 22.))
        .type_role(TypeRole::Body, cx)
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(size).h(size).rounded_full().bg(color)),
        )
        .child(div().flex_1().min_w_0().child(input))
}

fn point_field(text: &str, depth: u8, cx: &mut Context<OutlineView>) -> PointField {
    let input = field("", text, cx);
    let subscription = subscribe(&input, cx);
    PointField {
        depth,
        input,
        _subscription: subscription,
    }
}

fn field(placeholder: &str, text: &str, cx: &mut Context<OutlineView>) -> Entity<TextInputView> {
    let input = cx.new(|cx| {
        TextInputView::new(
            TextInputConfig::single_line(placeholder)
                .appearance(TextInputAppearance::Inline)
                .parent_navigation()
                .min_height(px(22.)),
            cx,
        )
    });
    if !text.is_empty() {
        input.update(cx, |field, cx| field.set_text(text, cx));
    }
    input
}

fn subscribe(input: &Entity<TextInputView>, cx: &mut Context<OutlineView>) -> Subscription {
    cx.subscribe(input, |this, source, event, cx| match event {
        TextInputEvent::Changed(_) => cx.emit(OutlineEvent::Changed),
        TextInputEvent::ParentKey {
            key,
            shift,
            platform,
        } => this.on_field_key(
            FieldKey {
                key,
                shift: *shift,
                source,
            },
            *platform,
            cx,
        ),
        TextInputEvent::Submit(_) | TextInputEvent::Cancel => {}
    })
}
