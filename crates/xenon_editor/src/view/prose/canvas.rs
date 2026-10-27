//! Prose canvas: layout preparation and painting.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Bounds, ElementInputHandler, Entity, FocusHandle, IntoElement, Pixels, RenderImage,
    Styled, Window, canvas, px,
};
use theme::ActiveTheme;
use xenon_settings::ProseFont;

use super::super::{Content, EditorView};
use super::*;
use crate::prose::layout::{self, LayoutInput, ProseLayout};
use crate::prose::paint::{self, PaintInput};
use crate::prose::projection::Projection;
use crate::prose::style;

pub(in crate::view) fn settings_hash(
    cx: &App,
    paper: bool,
    comments_open: bool,
    light: bool,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let s = xenon_settings::prose(cx);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (s.markers as u8, s.font as u8, paper, comments_open, light).hash(&mut h);
    s.size.to_bits().hash(&mut h);
    s.line_height.to_bits().hash(&mut h);
    s.measure.to_bits().hash(&mut h);
    let e = xenon_settings::editor_font(cx);
    (e.family.clone(), e.size.to_bits()).hash(&mut h);
    xenon_settings::ui_font(cx).family.hash(&mut h);
    h.finish()
}

pub(in crate::view) fn prose_canvas(
    view: Entity<EditorView>,
    focus: FocusHandle,
    register_input: bool,
) -> impl IntoElement {
    canvas(
        {
            let view = view.clone();
            move |bounds, window, cx| prepare(&view, bounds, window, cx)
        },
        move |bounds, prepared, window, cx| {
            let Some(prepared) = prepared else { return };
            let focused = focus.is_focused(window) && window.is_window_active();
            let (show, selection, caret, hide) = {
                let v = view.read(cx);
                let Content::Text(buffer) = &v.content else {
                    return;
                };
                (
                    !focused || v.cursor_blink.visible(),
                    buffer.selection_range(),
                    buffer.cursor(),
                    v.hide_markers(cx),
                )
            };
            paint::paint(
                &PaintInput {
                    layout: &prepared.layout,
                    palette: prepared.palette,
                    bounds,
                    scroll: prepared.scroll,
                    selection,
                    caret: Some(caret),
                    show_caret: show,
                    hide_markers: hide,
                },
                window,
                cx,
            );
            if register_input {
                window.handle_input(&focus, ElementInputHandler::new(bounds, view.clone()), cx);
            }
        },
    )
    .size_full()
}

pub(super) struct Prepared {
    layout: Rc<ProseLayout>,
    palette: style::Palette,
    scroll: Pixels,
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub(in crate::view) fn prepare(
    view: &Entity<EditorView>,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Prepared> {
    let theme = cx.theme().clone();
    let light = theme.appearance().is_light();
    let (paper, comments_open, mutation, rope, path, cursor) = {
        let v = view.read(cx);
        let Content::Text(buffer) = &v.content else {
            return None;
        };
        (
            v.prose.paper,
            v.prose.comments_open,
            buffer.mutation(),
            buffer.rope().clone(),
            buffer.path().to_path_buf(),
            buffer.cursor(),
        )
    };
    let palette = style::palette(theme.colors(), light, paper, theme.players().local().cursor);
    let settings = xenon_settings::prose(cx);
    let key = (
        mutation,
        f32::from(bounds.size.width).round() as u32,
        settings_hash(cx, paper, comments_open, light),
    );
    let cached = {
        let v = view.read(cx);
        (v.prose.layout_key == Some(key))
            .then(|| v.prose.layout.clone())
            .flatten()
    };
    let layout = match cached {
        Some(l) => l,
        None => {
            let hide = settings.markers == xenon_settings::ProseMarkers::Hide;
            let proj = Projection::build(&rope, comments_open, hide);
            let ui = xenon_settings::ui_font(cx);
            let editor = xenon_settings::editor_font(cx);
            let body_family: String = match settings.font {
                ProseFont::Mono => editor.family.clone(),
                ProseFont::Sans => ui.family.clone(),
                ProseFont::Serif => "Georgia".to_string(),
                ProseFont::Auto => {
                    if paper {
                        "Georgia".to_string()
                    } else {
                        ui.family.clone()
                    }
                }
            };
            let input = LayoutInput {
                width: bounds.size.width,
                size: px(settings.size),
                line_height: settings.line_height,
                measure: settings.measure,
                body_font: gpui::font(body_family),
                mono_font: crate::element::editor_font(&editor.family),
                palette,
                doc_dir: path.parent().map(|p| p.to_path_buf()),
                highlight: {
                    let theme = theme.clone();
                    let ink = palette.ink;
                    Box::new(move |lang, code| {
                        let syntax = theme.syntax();
                        crate::highlight::spans_for_lang(lang, code)
                            .into_iter()
                            .map(|span| {
                                let color = syntax
                                    .style_for_name(span.name)
                                    .and_then(|s| s.color)
                                    .unwrap_or(ink);
                                (span.start..span.end, color)
                            })
                            .collect()
                    })
                },
            };
            let mut images = view.update(cx, |v, _| std::mem::take(&mut v.prose.images));
            let mut loader = |p: &std::path::Path| -> Option<(Arc<RenderImage>, f32, f32)> {
                images
                    .entry(p.to_path_buf())
                    .or_insert_with(|| load_image(p))
                    .clone()
            };
            let laid = Rc::new(layout::layout(proj, &input, &mut loader, window));
            view.update(cx, |v, _| {
                v.prose.images = images;
                v.prose.layout = Some(laid.clone());
                v.prose.layout_key = Some(key);
            });
            laid
        }
    };
    let scroll = view.update(cx, |v, _| {
        v.prose.bounds = bounds;
        let max = (layout.height - bounds.size.height).max(px(0.));
        let mut scroll = v.prose.scroll.min(max);
        if v.prose.last_cursor != Some(cursor) {
            v.prose.last_cursor = Some(cursor);
            if let Some(c) = layout.caret(cursor) {
                let margin = c.h * 2.;
                if c.y < scroll + margin {
                    scroll = (c.y - margin).max(px(0.));
                } else if c.y + c.h > scroll + bounds.size.height - margin {
                    scroll = (c.y + c.h + margin - bounds.size.height)
                        .min(max)
                        .max(px(0.));
                }
            }
        }
        v.prose.scroll = scroll;
        scroll
    });
    Some(Prepared {
        layout,
        palette,
        scroll,
    })
}
