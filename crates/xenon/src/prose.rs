//! Markdown writing mode for an editor tab, built from stock GtkTextView
//! parts: text tags for the styles, the `invisible` tag for markup away from
//! the caret, view margins for the column, and line spacing for the line
//! height. The parsing and the "which markup shows" rule live in
//! `xenon_prose`; this file only moves them onto the buffer.
//!
//! Typing stays cheap: an edit only shifts the stored ranges and arms a
//! debounce timer. The reparse runs on a worker thread, and its result is
//! applied to the buffer in slices from idle callbacks, each slice replacing
//! the tags of its own range in one step so nothing flickers.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use sourceview::prelude::*;
use xenon_core::WorkspaceId;
use xenon_prose::{Doc, Span};
use xenon_settings::{ProseFont, ProseMarkers, ProseSettings};

use crate::app::App;
use crate::prose_tags::{Look, Tags};

mod code;
mod decor;
mod input;
mod style;
pub(crate) use style::css;

/// Quiet time after the last edit before the document is parsed again.
const DEBOUNCE: Duration = Duration::from_millis(120);
/// Spans applied per idle slice.
const SLICE: usize = 1500;

/// Is `path` a Markdown file the prose mode can show?
pub(crate) fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

pub(crate) struct Prose {
    pub(crate) view: sourceview::View,
    pub(crate) buffer: sourceview::Buffer,
    pub(crate) scroller: gtk::ScrolledWindow,
    pub(crate) path: PathBuf,
    pub(crate) ws: WorkspaceId,
    pub(crate) app: Weak<App>,
    tags: Tags,
    /// Whether prose mode is showing.
    pub(crate) on: Cell<bool>,
    hide: Cell<bool>,
    doc: RefCell<Doc>,
    /// Counts edits; a parse started at an older count is dropped.
    edits: Cell<u64>,
    /// Counts applications; a slice from an older one stops.
    applies: Cell<u64>,
    due: Cell<Option<Instant>>,
    timer: Cell<bool>,
    /// The block whose markup is showing (markers = hide).
    revealed: Cell<Option<usize>>,
    margin: Cell<i32>,
    /// Column width in characters, fitted to the document when it loads.
    column: Cell<f32>,
    pub(crate) images: crate::prose_images::Images,
    /// Overlay that draws quote bars, bullets and table rules.
    decor: gtk::DrawingArea,
    decor_pending: Cell<bool>,
    /// Tags made from the style scheme for code blocks, and their results.
    syntax: RefCell<Vec<gtk::TextTag>>,
    lit: RefCell<HashMap<String, code::Lit>>,
}

impl Prose {
    pub(crate) fn new(
        app: &Rc<App>,
        ws: WorkspaceId,
        view: &sourceview::View,
        scroller: &gtk::ScrolledWindow,
        path: &Path,
    ) -> Rc<Self> {
        let buffer = view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .expect("an editor view shows a source buffer");
        let (start, end) = buffer.bounds();
        let text = buffer.text(&start, &end, true);
        let this = Rc::new(Self {
            view: view.clone(),
            tags: Tags::new(&buffer.tag_table()),
            buffer,
            scroller: scroller.clone(),
            path: path.to_path_buf(),
            ws,
            app: Rc::downgrade(app),
            on: Cell::new(false),
            hide: Cell::new(false),
            doc: RefCell::default(),
            edits: Cell::new(0),
            applies: Cell::new(0),
            due: Cell::new(None),
            timer: Cell::new(false),
            revealed: Cell::new(None),
            margin: Cell::new(-1),
            column: Cell::new(xenon_prose::column_width(&text)),
            images: crate::prose_images::Images::default(),
            decor: gtk::DrawingArea::new(),
            decor_pending: Cell::new(false),
            syntax: RefCell::default(),
            lit: RefCell::default(),
        });
        this.connect();
        this
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.decor.set_can_target(false);
        self.decor.set_visible(false);
        self.view.add_overlay(&self.decor, 0, 0);
        self.decor.set_draw_func(glib::clone!(
            #[strong(rename_to = this)]
            weak,
            move |_, cr, _, _| {
                if let Some(this) = this.upgrade() {
                    this.draw_decor(cr);
                }
            }
        ));
        // Before the default handler: positions are those of the edit.
        self.buffer.connect_insert_text(glib::clone!(
            #[strong(rename_to = this)]
            weak,
            move |_, at, text| {
                if let Some(this) = this.upgrade() {
                    this.edited(at.offset() as usize, 0, text.chars().count());
                }
            }
        ));
        self.buffer.connect_delete_range(glib::clone!(
            #[strong(rename_to = this)]
            weak,
            move |_, start, end| {
                if let Some(this) = this.upgrade() {
                    let (a, b) = (start.offset() as usize, end.offset() as usize);
                    this.edited(a.min(b), a.abs_diff(b), 0);
                }
            }
        ));
        self.buffer.connect_cursor_position_notify(glib::clone!(
            #[strong(rename_to = this)]
            weak,
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.caret_moved();
                }
            }
        ));
        // The column follows the width of the view.
        self.scroller
            .hadjustment()
            .connect_page_size_notify(glib::clone!(
                #[strong(rename_to = this)]
                weak,
                move |_| {
                    if let Some(this) = this.upgrade() {
                        this.fit_column();
                    }
                }
            ));
        self.scroller
            .vadjustment()
            .connect_upper_notify(glib::clone!(
                #[strong(rename_to = this)]
                weak,
                move |_| {
                    if let Some(this) = this.upgrade() {
                        this.images.schedule_move(&this);
                        this.sync_decor();
                    }
                }
            ));
        let click = gtk::GestureClick::new();
        click.set_button(1);
        click.connect_released(glib::clone!(
            #[strong(rename_to = this)]
            weak,
            move |gesture, _, x, y| {
                let Some(this) = this.upgrade() else { return };
                let ctrl = gesture
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK);
                this.clicked(x, y, ctrl);
            }
        ));
        self.view.add_controller(click);
    }

    fn edited(self: &Rc<Self>, at: usize, removed: usize, inserted: usize) {
        if !self.on.get() {
            return;
        }
        self.edits.set(self.edits.get() + 1);
        self.doc.borrow_mut().shift(at, removed, inserted);
        self.arm(DEBOUNCE);
    }

    /// Parse once the buffer has been quiet for `wait`.
    fn arm(self: &Rc<Self>, wait: Duration) {
        self.due.set(Some(Instant::now() + wait));
        if self.timer.replace(true) {
            return;
        }
        self.tick(wait);
    }

    fn tick(self: &Rc<Self>, wait: Duration) {
        let this = Rc::downgrade(self);
        glib::timeout_add_local_once(wait, move || {
            let Some(this) = this.upgrade() else { return };
            let left = this
                .due
                .get()
                .map(|d| d.saturating_duration_since(Instant::now()));
            match left {
                Some(left) if !left.is_zero() => this.tick(left),
                _ => {
                    this.timer.set(false);
                    this.due.set(None);
                    this.parse();
                }
            }
        });
    }

    /// Parse a snapshot on a worker thread; apply it if nothing changed.
    fn parse(self: &Rc<Self>) {
        if !self.on.get() {
            return;
        }
        let (start, end) = self.buffer.bounds();
        // Hidden characters are part of the text.
        let text = self.buffer.text(&start, &end, true).to_string();
        let edits = self.edits.get();
        let this = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(doc) = gio::spawn_blocking(move || Doc::parse(&text)).await else {
                return;
            };
            if let Some(this) = this.upgrade()
                && this.on.get()
                && this.edits.get() == edits
            {
                this.apply(doc);
            }
        });
    }

    fn caret(&self) -> usize {
        self.buffer.cursor_position() as usize
    }

    fn apply(self: &Rc<Self>, doc: Doc) {
        let generation = self.applies.get() + 1;
        self.applies.set(generation);
        self.revealed.set(
            self.hide
                .get()
                .then(|| doc.block_at(self.caret()))
                .flatten(),
        );
        *self.doc.borrow_mut() = doc;
        self.slice(generation, 0, 0, Vec::new());
    }

    fn at(&self, offset: usize) -> gtk::TextIter {
        self.buffer.iter_at_offset(offset as i32)
    }

    fn tag_range(&self, tag: &gtk::TextTag, r: std::ops::Range<usize>) {
        if r.start < r.end {
            self.buffer
                .apply_tag(tag, &self.at(r.start), &self.at(r.end));
        }
    }

    /// Replace the tags of one stretch of the buffer, then queue the next.
    fn slice(self: &Rc<Self>, generation: u64, from: usize, a: usize, carry: Vec<Span>) {
        if self.applies.get() != generation || !self.on.get() {
            return;
        }
        let doc = self.doc.borrow();
        let n = doc.spans.len();
        let mut to = (from + SLICE).min(n);
        while to < n && to > from && doc.spans[to].range.start == doc.spans[to - 1].range.start {
            to += 1;
        }
        let total = self.buffer.char_count() as usize;
        let b = if to >= n {
            total
        } else {
            doc.spans[to].range.start.min(total)
        };
        let b = b.max(a);
        for tag in self.tags.all().into_iter().chain(&*self.syntax.borrow()) {
            self.buffer.remove_tag(tag, &self.at(a), &self.at(b));
        }
        let clip = |r: &std::ops::Range<usize>| r.start.max(a)..r.end.min(b);
        let mut next: Vec<Span> = Vec::new();
        for span in carry.iter().chain(&doc.spans[from..to]) {
            self.tag_range(self.tags.for_kind(span.kind), clip(&span.range));
            if span.range.end > b {
                next.push(span.clone());
            }
        }
        let shown = self.revealed.get();
        for ix in doc.markup_in(&(a..b)) {
            let range = clip(&doc.markup[ix].range);
            self.tag_range(&self.tags.dim, range.clone());
            if self.hide.get() && !doc.markup_shown(ix, shown) {
                self.tag_range(&self.tags.hide, range);
            }
        }
        for m in doc.bullets_in(&(a..b)) {
            if self.hide.get() && shown.is_none_or(|_| m.block != shown) {
                self.tag_range(&self.tags.bullet, clip(&m.range));
            }
        }
        self.color_code(&doc, a, b);
        drop(doc);
        if to < n {
            let this = Rc::downgrade(self);
            glib::idle_add_local_once(move || {
                if let Some(this) = this.upgrade() {
                    this.slice(generation, to, b, next);
                }
            });
        } else {
            self.images.rebuild(self);
            self.sync_decor();
        }
    }

    /// Show the markup of the block the caret entered; hide the block it left.
    fn caret_moved(self: &Rc<Self>) {
        if !self.on.get() || !self.hide.get() {
            return;
        }
        let doc = self.doc.borrow();
        let now = doc.block_at(self.caret());
        let before = self.revealed.get();
        if now == before {
            return;
        }
        self.revealed.set(now);
        if let Some(old) = before.and_then(|b| doc.blocks.get(b)) {
            for ix in doc.markup_in(old) {
                self.tag_range(&self.tags.hide, doc.markup[ix].range.clone());
            }
            for m in doc.bullets_in(old) {
                self.tag_range(&self.tags.bullet, m.range.clone());
            }
        }
        if let Some(new) = now.and_then(|b| doc.blocks.get(b)) {
            for ix in doc.markup_in(new) {
                let r = &doc.markup[ix].range;
                self.buffer
                    .remove_tag(&self.tags.hide, &self.at(r.start), &self.at(r.end));
            }
            for m in doc.bullets_in(new) {
                let r = &m.range;
                self.buffer
                    .remove_tag(&self.tags.bullet, &self.at(r.start), &self.at(r.end));
            }
        }
        drop(doc);
        self.sync_decor();
        self.images.schedule_move(self);
    }

    /// Fit the column to freshly loaded text.
    pub(crate) fn refit_column(self: &Rc<Self>) {
        let (start, end) = self.buffer.bounds();
        let text = self.buffer.text(&start, &end, true);
        self.column.set(xenon_prose::column_width(&text));
        self.margin.set(-1);
        self.fit_column();
    }

    /// Turn prose mode on or off for this view and restyle it.
    pub(crate) fn set_on(self: &Rc<Self>, on: bool, app: &App) {
        let was = self.on.replace(on);
        self.decor.set_visible(on);
        if on {
            self.restyle(app, !was);
        } else if was {
            self.applies.set(self.applies.get() + 1);
            let (a, b) = self.buffer.bounds();
            for tag in self.tags.all().into_iter().chain(&*self.syntax.borrow()) {
                self.buffer.remove_tag(tag, &a, &b);
            }
            self.images.clear(self);
            *self.doc.borrow_mut() = Doc::default();
            self.revealed.set(None);
            self.view.remove_css_class("xprose");
            self.view.set_left_margin(8);
            self.view.set_right_margin(12);
            self.view.set_pixels_above_lines(0);
            self.view.set_pixels_below_lines(0);
            self.view.set_pixels_inside_wrap(0);
            self.margin.set(-1);
        }
    }
}
