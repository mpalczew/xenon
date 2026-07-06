//! `TerminalView`: a focusable gpui view that owns a live PTY terminal and
//! draws its grid. The view exists immediately in a `Pending` state and swaps
//! to `Ready` when the PTY finishes spawning on the background executor.
//!
//! Spawn/subscribe patterns follow zed's terminal_view and project::terminals
//! (GPL-3.0-or-later); see ATTRIBUTION.md.

use std::path::PathBuf;

use anyhow::Result;
use collections::HashMap;
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, Render, Styled, Subscription, Task, Window,
    canvas, div,
};
use task::Shell;
use terminal::terminal_settings::{AlternateScroll, CursorShape};
use terminal::{Terminal, TerminalBuilder};
use theme::ActiveTheme;
use util::paths::PathStyle;

use crate::grid;

const LINE_HEIGHT_MULTIPLIER: f32 = 1.2;

enum State {
    Pending,
    Ready(Entity<Terminal>),
    Failed(String),
}

pub struct TerminalView {
    state: State,
    focus: FocusHandle,
    _spawn: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl TerminalView {
    /// Build the view immediately and spawn a `$SHELL` PTY at `working_dir` in
    /// the background; the view repaints itself when the terminal is ready.
    pub fn new(working_dir: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let builder = build(working_dir, cx);
        let spawn = cx.spawn(async move |view, cx| {
            let result = builder.await;
            view.update(cx, |view, cx| view.resolve(result, cx)).ok();
        });
        Self {
            state: State::Pending,
            focus: cx.focus_handle(),
            _spawn: spawn,
            _subscriptions: Vec::new(),
        }
    }

    fn resolve(&mut self, result: Result<TerminalBuilder>, cx: &mut Context<Self>) {
        match result {
            Ok(builder) => {
                let terminal = cx.new(|cx| builder.subscribe(cx));
                self._subscriptions
                    .push(cx.observe(&terminal, |_, _, cx| cx.notify()));
                self.state = State::Ready(terminal);
            }
            Err(error) => self.state = State::Failed(error.to_string()),
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state {
            terminal.update(cx, |terminal, _cx| {
                terminal.try_keystroke(&event.keystroke, false);
            });
            cx.notify();
        }
    }
}

fn build(working_dir: Option<PathBuf>, cx: &App) -> Task<Result<TerminalBuilder>> {
    TerminalBuilder::new(
        working_dir,
        None,
        Shell::System,
        HashMap::default(),
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().colors().terminal_background;
        let base = div()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .bg(background);

        match &self.state {
            State::Ready(terminal) => base.child(grid_canvas(terminal.clone())),
            State::Pending => base,
            State::Failed(error) => {
                base.text_color(cx.theme().colors().text).child(error.clone())
            }
        }
    }
}

fn grid_canvas(terminal: Entity<Terminal>) -> impl IntoElement {
    let font = grid::terminal_font();
    canvas(
        move |bounds, window, cx| layout(&terminal, &font, bounds, window, cx),
        |_bounds, lines, window, cx| {
            let line_height = grid::line_height(font_size(window), LINE_HEIGHT_MULTIPLIER);
            grid::paint(&lines, line_height, window, cx);
        },
    )
}

fn font_size(window: &Window) -> Pixels {
    window.text_style().font_size.to_pixels(window.rem_size())
}

fn layout(
    terminal: &Entity<Terminal>,
    font: &gpui::Font,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<grid::GridLine> {
    let size = font_size(window);
    let line_height = grid::line_height(size, LINE_HEIGHT_MULTIPLIER);
    grid::layout(terminal, bounds, font, size, line_height, window, cx)
}
