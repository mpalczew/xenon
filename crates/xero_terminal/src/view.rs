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
use std::time::Duration;

use anyhow::Result;
use gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollWheelEvent, StatefulInteractiveElement, Styled, Subscription, Task,
    UTF16Selection, Window, anchored, canvas, deferred, div, px,
};
use settings::Settings;
use task::Shell;
use terminal::terminal_settings::{AlternateScroll, CursorShape, TerminalSettings};
use terminal::{Terminal, TerminalBuilder};
use theme::ActiveTheme;
use util::paths::PathStyle;

use crate::clipboard::terminal_clipboard_text;
use crate::grid;

const LINE_HEIGHT_MULTIPLIER: f32 = 1.2;
const SCROLL_MULTIPLIER: f32 = 3.;

enum State {
    Pending,
    Ready(Entity<Terminal>),
    Failed(String),
}

/// Events a `TerminalView` emits upward; the shell subscribes to mark streams
/// that need attention (`Bell`) and clear the mark on use (`Interacted`).
pub enum TerminalEvent {
    Bell,
    Interacted,
    /// A busy Claude terminal went quiet: the agent likely finished its turn.
    Finished,
    /// The shell process exited; the terminal is dead but stays open.
    Exited,
    /// A file path was cmd-clicked in the terminal; open it in the editor.
    OpenPath(PathBuf),
    /// A cmd-clicked path-like token that did not resolve to a file on disk; the
    /// app fuzzy-matches it against the workspace index (carries the raw token,
    /// `:line:col` already stripped).
    ResolvePath(String),
}

/// The link under the cursor while Cmd is held: its matched text and where to
/// anchor the "⌘-click to open" tooltip. zed only resolves the hovered link while
/// the Cmd (secondary) modifier is held, so this is `Some` only then.
struct HoverInfo {
    label: String,
    position: Point<Pixels>,
}

/// Output must be quiet this long before a terminal counts as settled.
const IDLE_AFTER: Duration = Duration::from_millis(2500);
/// A settled burst below this many output batches is a short command, not an
/// agent working; only larger bursts flag attention.
const BUSY_WAKEUPS: u32 = 15;

pub struct TerminalView {
    state: State,
    focus: FocusHandle,
    focused_once: bool,
    /// Basename of the dir the terminal was spawned in; the title's cwd prefix is
    /// dropped when it still equals this (redundant with the sidebar).
    root_name: String,
    /// The shell exited; the view stays but is marked dead.
    exited: bool,
    /// Position of the right-click Copy/Paste menu, when open (window coords).
    context_menu: Option<Point<Pixels>>,
    /// Existing file resolved under the right-click, if any; adds an "Open in
    /// Browser" item to the context menu.
    menu_path: Option<PathBuf>,
    /// The link under the cursor while Cmd is held (drives the pointer cursor and
    /// the "⌘-click to open" tooltip).
    hovered_link: Option<HoverInfo>,
    _spawn: Task<()>,
    /// Output batches in the current burst; reset when output settles.
    wakeups: u32,
    /// Debounce that fires `on_idle` once output has been quiet for `IDLE_AFTER`.
    _idle_check: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl TerminalView {
    /// Build the view immediately and spawn a `$SHELL` PTY at `working_dir` in
    /// the background; the view repaints itself when the terminal is ready.
    pub fn new(
        working_dir: Option<PathBuf>,
        env: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let root_name = working_dir
            .as_deref()
            .and_then(|dir| dir.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let builder = build(working_dir, env, cx);
        let spawn = cx.spawn(async move |view, cx| {
            let result = builder.await;
            view.update(cx, |view, cx| view.resolve(result, cx)).ok();
        });
        Self {
            state: State::Pending,
            focus: cx.focus_handle(),
            focused_once: false,
            root_name,
            exited: false,
            context_menu: None,
            menu_path: None,
            hovered_link: None,
            _spawn: spawn,
            wakeups: 0,
            _idle_check: Task::ready(()),
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
            Event::Bell => {
                cx.emit(TerminalEvent::Bell);
                cx.notify();
            }
            Event::Wakeup => {
                self.wakeups = self.wakeups.saturating_add(1);
                self.arm_idle_check(cx);
                cx.notify();
            }
            Event::CloseTerminal => {
                self.exited = true;
                cx.emit(TerminalEvent::Exited);
                cx.notify();
            }
            Event::Open(target) => self.open_target(target, cx),
            Event::TitleChanged | Event::BreadcrumbsChanged => cx.notify(),
            _ => {}
        }
    }

    /// A cmd-click landed on a URL or path-like target in the terminal.
    fn open_target(&mut self, target: &terminal::MaybeNavigationTarget, cx: &mut Context<Self>) {
        match target {
            terminal::MaybeNavigationTarget::Url(url) => cx.open_url(url),
            terminal::MaybeNavigationTarget::PathLike(path_like) => {
                match resolve_clicked_path(path_like) {
                    Some(path) => cx.emit(TerminalEvent::OpenPath(path)),
                    // Not on disk relative to the PTY cwd (e.g. a bare `grid.rs`): let
                    // the app fuzzy-resolve it against the workspace index.
                    None => cx.emit(TerminalEvent::ResolvePath(
                        strip_line_suffix(&path_like.maybe_path).to_string(),
                    )),
                }
            }
        }
    }

    /// (Re)arm the finish detector: a debounce that fires once output has been
    /// quiet for `IDLE_AFTER`. An agent redraws its spinner continuously while
    /// working, so a gap that long means it stopped.
    fn arm_idle_check(&mut self, cx: &mut Context<Self>) {
        self._idle_check = cx.spawn(async move |view, cx| {
            cx.background_executor().timer(IDLE_AFTER).await;
            view.update(cx, |view, cx| view.on_idle(cx)).ok();
        });
    }

    /// Output settled after a burst. If that burst was substantial (an agent
    /// working, not a one-line command) and this is a Claude terminal, flag it.
    fn on_idle(&mut self, cx: &mut Context<Self>) {
        let busy = std::mem::replace(&mut self.wakeups, 0);
        if busy >= BUSY_WAKEUPS && self.title(cx).to_lowercase().contains("claude") {
            cx.emit(TerminalEvent::Finished);
        }
    }

    /// Whether the shell process has exited (the terminal is dead).
    pub fn is_exited(&self) -> bool {
        self.exited
    }

    /// The terminal's title (set by the program via OSC, e.g. Claude Code's
    /// status), for the terminal tab label.
    pub fn title(&self, cx: &App) -> String {
        match &self.state {
            State::Ready(terminal) => {
                // zed's default title is `{cwd} — {process}`. Drop the cwd prefix
                // only while it still equals the spawn dir (redundant with the
                // sidebar); a cd'd-into subdir is kept, as is a program-set title.
                let title = terminal.read(cx).title(false);
                let title = match title.split_once(" — ") {
                    Some((dir, rest)) if dir == self.root_name => rest,
                    _ => title.as_str(),
                };
                if title.is_empty() {
                    "terminal".into()
                } else {
                    title.to_string()
                }
            }
            State::Pending => "terminal".into(),
            State::Failed(_) => "error".into(),
        }
    }

    /// The user acted in this terminal: clear the attention mark and reset the
    /// finish detector, so their own keystroke echo can't be mistaken for an
    /// agent working (only output that arrives without interaction counts).
    fn note_interaction(&mut self, cx: &mut Context<Self>) {
        self.wakeups = 0;
        cx.emit(TerminalEvent::Interacted);
    }

    /// Write UTF-8 text straight to the PTY (used by the input handler).
    fn send_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        terminal.update(cx, |terminal, _| {
            terminal.input(text.to_string().into_bytes())
        });
        self.note_interaction(cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        self.note_interaction(cx);
        let keystroke = &event.keystroke;
        // Cmd-V pastes the clipboard; Cmd-C copies the selection.
        if keystroke.modifiers.platform && keystroke.key == "v" {
            self.paste_clipboard(cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if keystroke.modifiers.platform && keystroke.key == "c" {
            self.copy_selection(cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let handled = terminal.update(cx, |terminal, _| terminal.try_keystroke(keystroke, false));
        if handled {
            cx.stop_propagation();
        }
        cx.notify();
    }

    /// Copy the current selection to the clipboard (no-op without one).
    fn copy_selection(&self, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state
            && let Some(text) = terminal.read(cx).last_content().selection_text.clone()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Paste the clipboard into the terminal (bracketed-paste aware).
    fn paste_clipboard(&self, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state
            && let Some(item) = cx.read_from_clipboard()
            && let Some(text) = terminal_clipboard_text(item)
        {
            terminal.update(cx, |terminal, _| terminal.paste(&text));
        }
    }

    fn on_right_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(event.position);
        self.menu_path = self.path_under_cursor(event.position, cx);
        cx.stop_propagation();
        cx.notify();
    }

    /// Resolve an existing file at the grid cell under `position` (window coords),
    /// for the right-click "Open in Browser" item. zed's own path detector is
    /// private and Cmd-gated, so extract the token ourselves from `last_content`.
    fn path_under_cursor(&self, position: Point<Pixels>, cx: &App) -> Option<PathBuf> {
        let State::Ready(terminal) = &self.state else {
            return None;
        };
        let terminal = terminal.read(cx);
        let content = terminal.last_content();
        let bounds = &content.terminal_bounds;
        let local = position - bounds.bounds.origin;
        if local.x < px(0.) || local.y < px(0.) {
            return None;
        }
        let col = (local.x / bounds.cell_width()) as usize;
        let display_row = (local.y / bounds.line_height()) as i32;
        let line = display_row - content.display_offset as i32;
        let word = word_at(content, line, col)?;
        resolve_clicked_path(&terminal::PathLikeTarget {
            maybe_path: word,
            terminal_dir: terminal.working_directory(),
        })
    }

    fn dismiss_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_path = None;
        if self.context_menu.take().is_some() {
            cx.notify();
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        self.note_interaction(cx);
        terminal.update(cx, |terminal, cx| terminal.mouse_down(event, cx));
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        if event.pressed_button == Some(MouseButton::Left) {
            let region = terminal.read(cx).last_content().terminal_bounds.bounds;
            terminal.update(cx, |terminal, cx| terminal.mouse_drag(event, region, cx));
        } else {
            // Hover: zed resolves the link under the cursor only while Cmd is held,
            // populating `last_hovered_word`; grid.rs underlines it from there.
            terminal.update(cx, |terminal, cx| terminal.mouse_move(event, cx));
            self.refresh_hovered_link(event.position, cx);
        }
        cx.notify();
    }

    /// Sync the pointer/tooltip state from zed's `last_hovered_word` after a move.
    fn refresh_hovered_link(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        self.hovered_link = terminal
            .read(cx)
            .last_content()
            .last_hovered_word
            .as_ref()
            .map(|word| HoverInfo {
                label: word.word.clone(),
                position,
            });
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state {
            terminal.update(cx, |terminal, cx| terminal.mouse_up(event, cx));
            cx.notify();
        }
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        self.note_interaction(cx);
        terminal.update(cx, |terminal, _| {
            terminal.scroll_wheel(event, SCROLL_MULTIPLIER)
        });
        cx.notify();
    }
}

fn build(
    working_dir: Option<PathBuf>,
    env: Vec<(String, String)>,
    cx: &App,
) -> Task<Result<TerminalBuilder>> {
    // zed's terminal only detects file-path hyperlinks when given non-empty path
    // regexes and a non-zero timeout (URL detection is always on). Reuse the
    // defaults zed loads into the settings store rather than vendoring regexes.
    let settings = TerminalSettings::get_global(cx);
    let path_regexes = settings.path_hyperlink_regexes.clone();
    let path_timeout = settings.path_hyperlink_timeout_ms;
    TerminalBuilder::new(
        working_dir,
        None,
        Shell::System,
        env.into_iter().collect(),
        CursorShape::default(),
        AlternateScroll::On,
        None,
        path_regexes,
        path_timeout,
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
        let colors = cx.theme().colors().clone();
        let base = div()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .relative()
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .size_full()
            .bg(colors.terminal_background);
        // Pointer cursor while a link is hovered (Cmd held), matching zed.
        let base = if self.hovered_link.is_some() {
            base.cursor_pointer()
        } else {
            base
        };

        // A dim bar across the top once the shell has exited.
        let exited = self.exited.then(|| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .px_2()
                .py_1()
                .bg(colors.surface_background)
                .border_b_1()
                .border_color(colors.border)
                .text_xs()
                .text_color(colors.text_muted)
                .child("⊘ session ended — process exited")
        });

        let menu = self
            .context_menu
            .map(|position| self.render_context_menu(position, cx));
        let tooltip = self
            .hovered_link
            .as_ref()
            .map(|info| self.render_link_tooltip(info, cx));
        match &self.state {
            State::Ready(terminal) => base
                .child(grid_canvas(
                    terminal.clone(),
                    cx.entity(),
                    self.focus.clone(),
                ))
                .children(exited)
                .children(tooltip)
                .children(menu),
            State::Pending => base.children(menu),
            State::Failed(error) => base
                .text_color(cx.theme().colors().text)
                .child(error.clone()),
        }
    }
}

impl TerminalView {
    /// A small "⌘-click to open" hint anchored below-right of the cursor while a
    /// link is hovered. Non-occluding, so the terminal keeps receiving hover moves.
    fn render_link_tooltip(
        &self,
        info: &HoverInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let anchor = Point {
            x: info.position.x + px(12.),
            y: info.position.y + px(20.),
        };
        deferred(
            anchored().position(anchor).child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.elevated_surface_background)
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!("⌘-click to open  {}", info.label)),
            ),
        )
        .with_priority(1)
    }

    fn render_context_menu(
        &self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let item = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover))
                .child(label)
        };
        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(140.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background);
        // "Open in Browser" only when the right-click landed on an existing file.
        if let Some(path) = self.menu_path.clone() {
            menu_box = menu_box.child(item("menu-open-browser", "Open in Browser").on_click(
                cx.listener(move |this, _, _, cx| {
                    cx.open_url(&format!("file://{}", path.display()));
                    this.dismiss_menu(cx);
                }),
            ));
        }
        let menu_box = menu_box
            .child(
                item("menu-copy", "Copy").on_click(cx.listener(|this, _, _, cx| {
                    this.copy_selection(cx);
                    this.dismiss_menu(cx);
                })),
            )
            .child(
                item("menu-paste", "Paste").on_click(cx.listener(|this, _, _, cx| {
                    this.paste_clipboard(cx);
                    this.dismiss_menu(cx);
                })),
            );
        // A full-window scrim dismisses on any click; the menu occludes so its
        // own clicks don't reach it.
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .child(deferred(anchored().position(position).child(menu_box)).with_priority(1))
    }
}

/// Resolve a cmd-clicked path-like target to an existing file: strip any trailing
/// `:line[:col]` and resolve relative paths against the terminal's directory.
fn resolve_clicked_path(target: &terminal::PathLikeTarget) -> Option<PathBuf> {
    for candidate in [
        target.maybe_path.as_str(),
        strip_line_suffix(&target.maybe_path),
    ] {
        let mut path = PathBuf::from(candidate);
        if path.is_relative() {
            match &target.terminal_dir {
                Some(dir) => path = dir.join(path),
                None => continue,
            }
        }
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// The whitespace-delimited token at grid cell `(line, col)`, trimmed of wrapping
/// brackets/quotes and trailing sentence punctuation. Used to resolve a path under
/// a right-click; `resolve_clicked_path` then handles `:line:col` and existence.
fn word_at(content: &terminal::Content, line: i32, col: usize) -> Option<String> {
    let num_cols = content.terminal_bounds.num_columns();
    if col >= num_cols {
        return None;
    }
    let mut row = vec![' '; num_cols];
    for indexed in &content.cells {
        if indexed.point.line == line && indexed.point.column < num_cols {
            row[indexed.point.column] = indexed.cell.character();
        }
    }
    if row[col].is_whitespace() {
        return None;
    }
    let mut start = col;
    while start > 0 && !row[start - 1].is_whitespace() {
        start -= 1;
    }
    let mut end = col;
    while end + 1 < num_cols && !row[end + 1].is_whitespace() {
        end += 1;
    }
    let token: String = row[start..=end].iter().collect();
    trim_token(&token)
}

/// Strip wrapping brackets/quotes and trailing sentence punctuation from a token,
/// keeping leading `./` and `/`, mirroring zed's default path-hyperlink boundaries
/// (e.g. `Added foo.html.` yields `foo.html`; `./rel` stays `./rel`).
fn trim_token(token: &str) -> Option<String> {
    let trimmed = token
        .trim_start_matches(|c: char| "([{<\"'`".contains(c))
        .trim_end_matches(|c: char| ")]}>\"'`.,;:".contains(c));
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Drop up to two trailing `:<digits>` segments (line and column) from a path.
fn strip_line_suffix(text: &str) -> &str {
    let mut text = text;
    for _ in 0..2 {
        match text.rsplit_once(':') {
            Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
                text = head;
            }
            _ => break,
        }
    }
    text
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
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
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

#[cfg(test)]
mod tests {
    use super::{strip_line_suffix, trim_token};

    #[test]
    fn trim_token_drops_trailing_sentence_period() {
        // "Added favicon to amazon-summary-2026.html." -> the bare filename.
        assert_eq!(
            trim_token("amazon-summary-2026.html.").as_deref(),
            Some("amazon-summary-2026.html")
        );
    }

    #[test]
    fn trim_token_keeps_relative_and_absolute_prefixes() {
        assert_eq!(
            trim_token("./rel/path.rs").as_deref(),
            Some("./rel/path.rs")
        );
        assert_eq!(trim_token("/abs/path.md").as_deref(), Some("/abs/path.md"));
    }

    #[test]
    fn trim_token_strips_wrapping_delimiters_but_keeps_line_col() {
        assert_eq!(trim_token("(foo.rs:12)").as_deref(), Some("foo.rs:12"));
        assert_eq!(trim_token("\"quoted\"").as_deref(), Some("quoted"));
    }

    #[test]
    fn trim_token_all_punctuation_is_none() {
        assert_eq!(trim_token("..."), None);
        assert_eq!(trim_token(""), None);
    }

    #[test]
    fn strip_line_suffix_removes_line_and_column() {
        assert_eq!(strip_line_suffix("foo.rs:12:3"), "foo.rs");
        assert_eq!(strip_line_suffix("foo.rs:12"), "foo.rs");
        assert_eq!(strip_line_suffix("foo.rs"), "foo.rs");
        // A non-numeric ":" tail is part of the path, not a line suffix.
        assert_eq!(strip_line_suffix("foo:bar"), "foo:bar");
    }
}
