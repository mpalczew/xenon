//! Prose mode glue: state, canvas, keys, mouse and the status strip. The
//! projection modules under `crate::prose` decide; this file wires them to the
//! one buffer and to GPUI.

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Bounds, Context, Pixels, RenderImage, px};
use xenon_settings::MarkdownMode;

use super::{Content, EditorView};
use crate::edit::{EditCommand, Motion};
use crate::prose::layout::{Hit, ProseLayout};
use crate::prose::projection::Projection;

type LoadedImage = Option<(Arc<RenderImage>, f32, f32)>;

pub(crate) struct ProseUi {
    pub mode: MarkdownMode,
    pub paper: bool,
    pub comments_open: bool,
    pub chrome_hidden: bool,
    pub scroll: Pixels,
    pub layout: Option<Rc<ProseLayout>>,
    pub layout_key: Option<(u64, u32, u64)>,
    pub bounds: Bounds<Pixels>,
    pub goal_x: Option<Pixels>,
    last_cursor: Option<usize>,
    /// Split mode: keys act on the prose pane (else the source pane).
    pub active_prose: bool,
    pub marked: Option<Range<usize>>,
    typing: bool,
    images: HashMap<PathBuf, LoadedImage>,
    pub dragging: bool,
    loaded_prefs: bool,
    words: Option<(u64, usize)>,
}

impl Default for ProseUi {
    fn default() -> Self {
        Self {
            mode: MarkdownMode::Prose,
            paper: true,
            comments_open: false,
            chrome_hidden: false,
            scroll: px(0.),
            layout: None,
            layout_key: None,
            bounds: Bounds::default(),
            goal_x: None,
            last_cursor: None,
            active_prose: true,
            marked: None,
            typing: false,
            images: HashMap::new(),
            dragging: false,
            loaded_prefs: false,
            words: None,
        }
    }
}

fn load_image(path: &std::path::Path) -> LoadedImage {
    let decoded = image::open(path).ok()?;
    let mut rgba = decoded.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    let frame = image::Frame::new(rgba);
    Some((Arc::new(RenderImage::new(vec![frame])), w as f32, h as f32))
}

impl EditorView {
    fn hide_markers(&self, cx: &App) -> bool {
        xenon_settings::prose_markers(cx) == xenon_settings::ProseMarkers::Hide
    }

    fn fresh_projection(&self, cx: &App) -> Option<Projection> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        Some(Projection::build(
            buffer.rope(),
            self.prose.comments_open,
            self.hide_markers(cx),
        ))
    }

    pub(super) fn end_typing(&mut self) {
        if self.prose.typing {
            self.prose.typing = false;
            if let Content::Text(buffer) = &mut self.content {
                buffer.set_undo_group(false);
            }
        }
    }

    pub(super) fn word_stats(&mut self, cx: &App) -> (usize, usize) {
        let Content::Text(buffer) = &self.content else {
            return (0, 0);
        };
        let key = buffer.mutation();
        let words = match self.prose.words {
            Some((k, w)) if k == key => w,
            _ => {
                let proj = Projection::build(buffer.rope(), true, false);
                let w = proj.words();
                self.prose.words = Some((key, w));
                w
            }
        };
        let _ = cx;
        (words, words.div_ceil(230).max(1))
    }

    fn after_prose_edit(&mut self, cx: &mut Context<Self>) {
        self.prose.goal_x = None;
        self.recompute_highlights();
        self.emit_buffer_changed(cx);
        self.emit_cursor(cx);
        self.cursor_blink.reset(cx, Self::blink_tick);
        cx.notify();
    }

    /// Apply one plain source edit and leave the caret after it.
    fn prose_replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.replace_range(range, text);
        self.after_prose_edit(cx);
    }

    /// Typed text in prose mode: words coalesce into one undo step.
    pub(super) fn prose_type(&mut self, text: &str, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let has_selection = buffer.selection_range().is_some();
        let mut chars = text.chars();
        let word_char =
            matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphanumeric());
        if has_selection {
            self.end_typing();
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if word_char && !self.prose.typing {
            buffer.set_undo_group(true);
            self.prose.typing = true;
        }
        buffer.replace_selection(text);
        if !word_char {
            self.end_typing();
        }
        self.after_prose_edit(cx);
    }

    fn rope_len(&self) -> usize {
        match &self.content {
            Content::Text(b) => b.rope().len_chars(),
            _ => 0,
        }
    }

    fn rope(&self) -> &ropey::Rope {
        match &self.content {
            Content::Text(b) => b.rope(),
            _ => unreachable!("prose runs on text buffers"),
        }
    }
}

mod canvas;
mod keys;
mod modes;
mod motion;
mod pointer;
mod strip;
mod toggles;
