//! Read-only diff tab: one file, HEAD against the working tree (or the staged
//! and unstaged halves when the two match). The model decides layout; this
//! view loads it off the UI thread, keeps scroll and focus, and draws.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Pixels, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, canvas, div, point, prelude::FluentBuilder,
    px,
};
use lucide_icons::Icon;
use theme::ActiveTheme;

use crate::diff_model::{DiffDoc, Layout, LayoutMode};
use crate::diff_rows::{self, Paint};
use crate::git_dirt::GitFile;
use crate::icons::icon;

pub(crate) struct DiffView {
    root: PathBuf,
    file: GitFile,
    mode: LayoutMode,
    doc: Option<Arc<DiffDoc>>,
    /// Pane width from the last paint; Auto resolves against it.
    width: f32,
    focus: FocusHandle,
    scroll: ScrollHandle,
    load: Option<Task<()>>,
}

impl DiffView {
    pub(crate) fn new(root: PathBuf, file: GitFile, cx: &mut Context<Self>) -> Self {
        let mut view = Self::bare(root, file, cx);
        view.reload(None, cx);
        view
    }

    /// A view with its diff already in hand (visual scenes).
    #[cfg(feature = "visual-tests")]
    pub(crate) fn with_doc(
        file: GitFile,
        mode: LayoutMode,
        doc: DiffDoc,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::bare(PathBuf::new(), file, cx);
        view.mode = mode;
        view.doc = Some(Arc::new(doc));
        view
    }

    fn bare(root: PathBuf, file: GitFile, cx: &mut Context<Self>) -> Self {
        Self {
            root,
            file,
            mode: LayoutMode::Auto,
            doc: None,
            width: 0.,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            load: None,
        }
    }

    pub(crate) fn path(&self) -> &str {
        &self.file.path
    }

    /// Re-read the diff. `file` carries a fresher status entry when the
    /// caller has one.
    pub(crate) fn reload(&mut self, file: Option<GitFile>, cx: &mut Context<Self>) {
        if let Some(file) = file {
            self.file = file;
        }
        let (root, file) = (self.root.clone(), self.file.clone());
        self.load = Some(cx.spawn(async move |this, cx| {
            let doc = cx
                .background_executor()
                .spawn(async move { crate::diff_load::load(&root, &file) })
                .await;
            this.update(cx, |this, cx| {
                this.doc = Some(Arc::new(doc));
                cx.notify();
            })
            .ok();
        }));
    }

    pub(crate) fn set_mode(&mut self, mode: LayoutMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub(crate) fn cycle_layout(&mut self, cx: &mut Context<Self>) {
        self.set_mode(self.mode.next(), cx);
    }

    fn layout(&self) -> Layout {
        self.mode.resolve(self.width)
    }

    fn scroll_by(&self, delta: Pixels) {
        let max = self.scroll.max_offset().y.max(px(0.));
        let mut offset = self.scroll.offset();
        offset.y = (offset.y - delta).clamp(-max, px(0.));
        self.scroll.set_offset(offset);
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let page = self.scroll.bounds().size.height * 0.9;
        let line = px(48.);
        match event.keystroke.key.as_str() {
            "down" | "j" => self.scroll_by(line),
            "up" | "k" => self.scroll_by(-line),
            "pagedown" | "space" => self.scroll_by(page),
            "pageup" => self.scroll_by(-page),
            "home" => self.scroll.set_offset(point(px(0.), px(0.))),
            "end" => self.scroll_by(self.scroll.max_offset().y),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn header(&self, layout: Layout, paint: &Paint, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &paint.colors;
        let (dir, name) = match self.file.path.rsplit_once('/') {
            Some((dir, name)) => (format!("{dir}/"), name.to_string()),
            None => (String::new(), self.file.path.clone()),
        };
        let subtitle = self
            .doc
            .as_ref()
            .map(|doc| doc.subtitle.clone())
            .unwrap_or_default();
        let (added, removed) = self.doc.as_ref().map_or((0, 0), |d| (d.added, d.removed));
        let resolved = (self.mode == LayoutMode::Auto && self.width > 0.).then(|| {
            let name = if layout == Layout::Split {
                "split"
            } else {
                "unified"
            };
            format!("→ {name} · {}px", self.width.round() as i32)
        });
        // Path (dimmed directory, strong name) as one truncating text run.
        let path_text = format!("{dir}{name}");
        let dir_len = dir.len();
        let path = gpui::StyledText::new(path_text.clone()).with_highlights([
            (
                0..dir_len,
                gpui::HighlightStyle {
                    color: Some(colors.text_muted),
                    ..Default::default()
                },
            ),
            (
                dir_len..path_text.len(),
                gpui::HighlightStyle {
                    color: Some(colors.text),
                    font_weight: Some(gpui::FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ),
        ]);
        let narrow = self.width > 0. && self.width < 720.;
        div()
            .flex()
            .flex_none()
            .when(narrow, |h| h.flex_col().items_start())
            .when(!narrow, |h| h.items_center())
            .gap_x_3()
            .gap_y_1()
            .min_h(px(40.))
            .px_4()
            .py_1()
            .border_b_1()
            .border_color(colors.border)
            .text_color(colors.text_muted)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .when(narrow, |g| g.w_full())
                    .overflow_hidden()
                    .items_center()
                    .gap_3()
                    .child(div().flex_none().whitespace_nowrap().child(path))
                    .child(div().flex_none().whitespace_nowrap().child(subtitle)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_3()
                    .child(self.layout_control(cx))
                    .children(resolved.map(|text| div().flex_none().child(text)))
                    .child(diff_rows::counts(added, removed, paint).flex_none())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(colors.border)
                            .child(icon(Icon::Lock, px(12.)))
                            .child("Read-only"),
                    ),
            )
    }

    /// Auto / Split / Unified. Clicking picks one; ⌘⌥⇧D cycles.
    fn layout_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let segments = [LayoutMode::Auto, LayoutMode::Split, LayoutMode::Unified];
        div()
            .flex()
            .flex_none()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .children(segments.into_iter().map(|mode| {
                let selected = mode == self.mode;
                div()
                    .id(SharedString::from(format!("diff-mode-{}", mode.label())))
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .when(selected, |s| {
                        s.bg(colors.element_selected)
                            .text_color(colors.text)
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .hover(|s| s.bg(colors.element_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_mode(mode, cx)))
                    .child(mode.label())
            }))
    }

    fn body(&self, layout: Layout, paint: &Paint, cx: &App) -> impl IntoElement {
        let colors = &paint.colors;
        let Some(doc) = &self.doc else {
            return div().p_4().text_color(colors.text_muted).child("Loading…");
        };
        let banner = doc.banner.as_ref().map(|text| {
            div()
                .mx_4()
                .mt_3()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.element_background)
                .child(text.clone())
        });
        let note = doc.note.as_ref().map(|text| {
            div()
                .p_4()
                .text_color(colors.text_muted)
                .child(text.clone())
        });
        let cut = doc.truncated.then(|| {
            div()
                .p_4()
                .text_color(colors.text_muted)
                .child("Diff truncated; open the file to see the rest.")
        });
        diff_rows::body_text(div().pb_4(), paint)
            .children(banner)
            .children(note)
            .children(
                doc.sections
                    .iter()
                    .map(|s| diff_rows::section(s, layout, paint, cx)),
            )
            .children(cut)
    }
}

impl Focusable for DiffView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paint = Paint::new(cx);
        let layout = self.layout();
        let entity = cx.entity();
        let width_probe = canvas(
            move |bounds, _, cx| {
                let width = f32::from(bounds.size.width);
                entity.update(cx, |view, cx| {
                    if (view.width - width).abs() >= 1. {
                        view.width = width;
                        cx.notify();
                    }
                });
            },
            |_, (), _, _| {},
        )
        .absolute()
        .size_full();
        div()
            .id("diff-view")
            .key_context("DiffView")
            .track_focus(&self.focus)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .bg(paint.colors.editor_background)
            .text_color(paint.colors.text)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                }),
            )
            .child(width_probe)
            .child(self.header(layout, &paint, cx))
            .child(
                div()
                    .id("diff-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(self.body(layout, &paint, cx)),
            )
    }
}
