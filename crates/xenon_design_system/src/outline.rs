//! Title plus indented points. The caller stores and validates the text.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div, px,
};
use theme::ActiveTheme;

use crate::outline_text::{OutlinePoint, details_from_points, points_from_details};
use crate::text_input::{TextInputAppearance, TextInputConfig, TextInputEvent, TextInputView};
use crate::typography::{TypeRole, Typography};

pub enum OutlineEvent {
    Changed,
    Submit,
    Cancel,
}

struct PointField {
    depth: u8,
    input: Entity<TextInputView>,
    _subscription: Subscription,
}

pub struct OutlineView {
    title: Entity<TextInputView>,
    points: Vec<PointField>,
    _title_subscription: Subscription,
}

impl EventEmitter<OutlineEvent> for OutlineView {}

impl OutlineView {
    pub fn new(title: &str, details: &str, cx: &mut Context<Self>) -> Self {
        let title = field("Name the work item", title, cx);
        let _title_subscription = subscribe(&title, cx);
        let points = points_from_details(details)
            .into_iter()
            .map(|point| point_field(&point.text, point.depth, cx))
            .collect();
        Self {
            title,
            points,
            _title_subscription,
        }
    }

    pub fn title(&self, cx: &App) -> String {
        self.title.read(cx).text().to_owned()
    }

    pub fn details(&self, cx: &App) -> String {
        details_from_points(&self.points(cx))
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

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.title.update(cx, |input, cx| input.set_text("", cx));
        self.points.clear();
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
            .flex()
            .flex_col()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .child(line(TypeRole::ListPrimary, 0, "☐", self.title.clone(), cx))
            .children(points.into_iter().map(|(depth, input)| {
                let mark = if depth == 0 { "•" } else { "◦" };
                line(TypeRole::Body, depth, mark, input, cx)
            }))
    }
}

fn line(
    role: TypeRole,
    depth: u8,
    mark: &'static str,
    input: Entity<TextInputView>,
    cx: &App,
) -> impl IntoElement {
    let accent = cx.theme().colors().text_accent;
    div()
        .flex()
        .items_center()
        .gap_2()
        .min_h(px(32.))
        .pl(px(f32::from(depth) * 22.))
        .type_role(role, cx)
        .child(div().w(px(17.)).text_color(accent).child(mark))
        .child(input)
}

fn point_field(text: &str, depth: u8, cx: &mut Context<OutlineView>) -> PointField {
    let input = field("One short point", text, cx);
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
                .parent_navigation(),
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
