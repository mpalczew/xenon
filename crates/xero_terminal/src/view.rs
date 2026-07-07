//! `TerminalView`: a focusable gpui view that owns a live PTY terminal and
//! draws its grid. The view exists immediately in a `Pending` state and swaps
//! to `Ready` when the PTY finishes spawning on the background executor.
//!
//! Text input flows through an `EntityInputHandler` (registered each paint), so
//! plain typing and IME reach the PTY; control/navigation keys go through
//! `try_keystroke` on key-down. Both mirror zed's terminal_view
//! (GPL-3.0-or-later); see ATTRIBUTION.md.

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use collections::HashMap;
use gpui::{
    App, AppContext, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render,
    ScrollWheelEvent, Styled, Subscription, Task, UTF16Selection, Window, canvas, div, px,
};
use task::Shell;
use terminal::terminal_settings::{AlternateScroll, CursorShape};
use terminal::{Terminal, TerminalBuilder};
use theme::ActiveTheme;
use util::paths::PathStyle;

use crate::grid;

const LINE_HEIGHT_MULTIPLIER: f32 = 1.2;
const SCROLL_MULTIPLIER: f32 = 3.;

enum State {
    Pending,
    Ready(Entity<Terminal>),
    Failed(String),
}

pub struct TerminalView {
    state: State,
    focus: FocusHandle,
    focused_once: bool,
    _spawn: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl TerminalView {
    /// Build the view immediately and spawn a `$SHELL` PTY at `working_dir` in
    /// the background; the view repaints itself when the terminal is ready.
    pub fn new(
        working_dir: Option<PathBuf>,
        env: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let builder = build(working_dir, env, cx);
        let spawn = cx.spawn(async move |view, cx| {
            let result = builder.await;
            view.update(cx, |view, cx| view.resolve(result, cx)).ok();
        });
        Self {
            state: State::Pending,
            focus: cx.focus_handle(),
            focused_once: false,
            _spawn: spawn,
            _subscriptions: Vec::new(),
        }
    }

    fn resolve(&mut self, result: Result<TerminalBuilder>, cx: &mut Context<Self>) {
        match result {
            Ok(builder) => {
                let terminal = cx.new(|cx| builder.subscribe(cx));
                // The PTY loop emits `Wakeup` on new output; it does not call
                // `notify`, so we repaint here or output lags to the next frame.
                self._subscriptions
                    .push(cx.subscribe(&terminal, Self::on_terminal_event));
                self.state = State::Ready(terminal);
            }
            Err(error) => self.state = State::Failed(error.to_string()),
        }
        cx.notify();
    }

    fn on_terminal_event(
        &mut self,
        _terminal: Entity<Terminal>,
        event: &terminal::Event,
        cx: &mut Context<Self>,
    ) {
        use terminal::Event;
        match event {
            Event::Wakeup | Event::TitleChanged | Event::BreadcrumbsChanged | Event::Bell => {
                cx.notify()
            }
            _ => {}
        }
    }

    /// Write UTF-8 text straight to the PTY (used by the input handler).
    fn send_text(&self, text: &str, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state
            && !text.is_empty()
        {
            terminal.update(cx, |terminal, _| terminal.input(text.to_string().into_bytes()));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state {
            let handled =
                terminal.update(cx, |terminal, _| terminal.try_keystroke(&event.keystroke, false));
            if handled {
                cx.stop_propagation();
            }
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state {
            terminal.update(cx, |terminal, _| terminal.scroll_wheel(event, SCROLL_MULTIPLIER));
            cx.notify();
        }
    }
}

fn build(
    working_dir: Option<PathBuf>,
    env: Vec<(String, String)>,
    cx: &App,
) -> Task<Result<TerminalBuilder>> {
    TerminalBuilder::new(
        working_dir,
        None,
        Shell::System,
        env.into_iter().collect(),
        CursorShape::default(),
        AlternateScroll::On,
        None,
        Vec::new(),
        0,
        false,
        0,
        None,
        cx,
        Vec::new(),
        PathStyle::local(),
    )
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let background = cx.theme().colors().terminal_background;
        let base = div()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(background);

        match &self.state {
            State::Ready(terminal) => {
                base.child(grid_canvas(terminal.clone(), cx.entity(), self.focus.clone()))
            }
            State::Pending => base,
            State::Failed(error) => {
                base.text_color(cx.theme().colors().text).child(error.clone())
            }
        }
    }
}

fn grid_canvas(
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    focus: FocusHandle,
) -> impl IntoElement {
    let font = grid::terminal_font();
    canvas(
        move |bounds, window, cx| layout(&terminal, &font, bounds, window, cx),
        move |bounds, grid_layout, window, cx| {
            let size = px(xero_settings::font_size(cx));
            let line_height = grid::line_height(size, LINE_HEIGHT_MULTIPLIER);
            grid::paint(&grid_layout, line_height, window, cx);
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .size_full()
}

fn layout(
    terminal: &Entity<Terminal>,
    font: &gpui::Font,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> grid::GridLayout {
    let size = px(xero_settings::font_size(cx));
    let line_height = grid::line_height(size, LINE_HEIGHT_MULTIPLIER);
    grid::layout(terminal, bounds, font, size, line_height, window, cx)
}

/// macOS text input: only `replace_text_in_range` is needed for plain typing;
/// the rest satisfy the protocol. IME composition is not handled in v1.
impl EntityInputHandler for TerminalView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_text(new_text, cx);
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
