//! `TerminalView`: a focusable gpui view that owns a live PTY terminal and
//! draws its grid. The view exists immediately in a `Pending` state and swaps
//! to `Ready` when the PTY finishes spawning on the background executor.
//!
//! Text input flows through an `EntityInputHandler` (registered each paint), so
//! plain typing and IME reach the PTY; control/navigation keys go through
//! `try_keystroke` on key-down. Both mirror zed's terminal_view
//! (GPL-3.0-or-later); see ATTRIBUTION.md.
//!
//! Xenon-only pieces live in submodules (`attention`, `paths`, find) so adapted
//! zed patterns stay easier to re-sync.

mod attention;
mod find_bar;
mod find_session;
mod paths;

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use attention::{IDLE_AFTER, agent_finish_signal};
use find_session::FindSession;
use gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, ExternalPaths, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Task, UTF16Selection, Window, actions,
    anchored, canvas, deferred, div, px,
};
use paths::{resolve_clicked_path, strip_line_suffix, word_at};
use settings::Settings;
use task::Shell;
use terminal::terminal_settings::{AlternateScroll, CursorShape, TerminalSettings};
use terminal::{Terminal, TerminalBuilder};
use theme::ActiveTheme;
use util::paths::PathStyle;

use crate::clipboard::{terminal_clipboard_text, terminal_paths_text};
use crate::grid;
use xenon_settings::{Copy, Cut, Paste, TerminalAutoClose};

actions!(xenon_terminal, [Find, FindNext, FindPrevious]);

const LINE_HEIGHT_MULTIPLIER: f32 = 1.2;
const SCROLL_MULTIPLIER: f32 = 3.;

struct MouseChipTooltip {
    text: SharedString,
}

impl Render for MouseChipTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .text_color(colors.text)
            .text_sm()
            .child(self.text.clone())
    }
}

/// Host-select mode while the TUI has mouse reporting: inject shift so zed
/// skips mouse reports. Must NOT be used on mouse-down for a new selection —
/// shift+simple is "extend only" and never starts a selection.
fn inject_shift_for_host_drag(mouse_to_app: bool, reporting: bool, alt: bool) -> bool {
    reporting && !mouse_to_app && !alt
}

enum State {
    Pending,
    Ready(Entity<Terminal>),
    Failed(String),
}

/// Events a `TerminalView` emits upward; the shell subscribes to mark workspaces
/// that need attention (`Bell`) and clear the mark on use (`Interacted`).
pub enum TerminalEvent {
    Bell,
    Interacted,
    /// A busy Claude terminal went quiet: the agent likely finished its turn.
    Finished,
    /// The shell process exited; the terminal is dead but stays open.
    Exited,
    /// Per-tab "close when process exits" policy changed (may reschedule close).
    AutoCloseChanged,
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

pub struct TerminalView {
    state: State,
    focus: FocusHandle,
    focused_once: bool,
    /// Basename of the dir the terminal was spawned in; the title's cwd prefix is
    /// dropped when it still equals this (redundant with the sidebar).
    root_name: String,
    /// The shell exited; the view stays but is marked dead.
    exited: bool,
    /// What to do when this tab's process exits (seeded from app settings).
    auto_close: TerminalAutoClose,
    /// Bumped when `auto_close` changes so pending delayed closes abort.
    auto_close_token: u64,
    /// Text to write once the PTY becomes Ready (task inject races spawn).
    pending_inject: Option<String>,
    /// Position of the right-click Copy/Paste menu, when open (window coords).
    context_menu: Option<Point<Pixels>>,
    /// Existing file resolved under the right-click, if any; adds an "Open in
    /// Browser" item to the context menu.
    menu_path: Option<PathBuf>,
    /// The link under the cursor while Cmd is held (drives the pointer cursor and
    /// the "⌘-click to open" tooltip).
    hovered_link: Option<HoverInfo>,
    /// When the TUI enables mouse reporting: true = pass mouse to the app
    /// (default), false = host text selection. Toggled from hover chrome.
    mouse_to_app: bool,
    /// Top hover chrome is open (thin hit zone expands into the full bar).
    chrome_hovered: bool,
    /// cmd-f find bar (scrollback search via alacritty RegexSearch).
    find: Option<FindSession>,
    /// In-flight find_matches task (dropped on next rescan / close).
    _find_task: Task<()>,
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
            auto_close: xenon_settings::terminal_auto_close(cx),
            auto_close_token: 0,
            pending_inject: None,
            context_menu: None,
            menu_path: None,
            hovered_link: None,
            mouse_to_app: true,
            chrome_hovered: false,
            find: None,
            _find_task: Task::ready(()),
            _spawn: spawn,
            wakeups: 0,
            _idle_check: Task::ready(()),
            _subscriptions: Vec::new(),
        }
    }

    pub fn auto_close(&self) -> TerminalAutoClose {
        self.auto_close
    }

    pub fn auto_close_token(&self) -> u64 {
        self.auto_close_token
    }

    /// Cycle On-exit policy for this tab only (does not change global settings).
    pub fn cycle_auto_close(&mut self, cx: &mut Context<Self>) {
        self.set_auto_close(self.auto_close.next(), cx);
    }

    pub fn set_auto_close(&mut self, mode: TerminalAutoClose, cx: &mut Context<Self>) {
        if self.auto_close == mode {
            return;
        }
        self.auto_close = mode;
        self.auto_close_token = self.auto_close_token.wrapping_add(1);
        cx.emit(TerminalEvent::AutoCloseChanged);
        cx.notify();
    }

    /// True while the program has enabled terminal mouse reporting (TUI).
    pub fn mouse_reporting(&self, cx: &App) -> bool {
        match &self.state {
            State::Ready(terminal) => terminal.read(cx).mouse_mode(false),
            _ => false,
        }
    }

    /// When reporting is on: true means plain drag goes to the TUI.
    pub fn mouse_to_app(&self) -> bool {
        self.mouse_to_app
    }

    /// Flip host-select vs app-mouse while a TUI has mouse reporting on.
    pub fn toggle_mouse_to_app(&mut self, cx: &mut Context<Self>) {
        self.mouse_to_app = !self.mouse_to_app;
        cx.notify();
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
                // Run Task often adds a tab then injects immediately; flush what
                // was queued while the PTY was still spawning.
                if let Some(text) = self.pending_inject.take() {
                    self.send_text(&text, cx);
                }
            }
            Err(error) => {
                self.pending_inject = None;
                self.state = State::Failed(error.to_string());
            }
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
                // Scrollback moved; refresh match ranges without jumping.
                if self.find_is_open() {
                    self.rescan_find(false, cx);
                }
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

    /// Output settled after a burst. If that burst was substantial (agent-like
    /// thrash, not a one-line command), flag the stream for attention.
    fn on_idle(&mut self, cx: &mut Context<Self>) {
        let busy = std::mem::replace(&mut self.wakeups, 0);
        let title = self.title(cx);
        if agent_finish_signal(busy, &title) {
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

    /// Inject text into the PTY (e.g. a resolved shell task line). Queues while
    /// the PTY is still spawning so Run Task on a new tab does not drop the line.
    pub fn inject_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }
        match &self.state {
            State::Ready(_) => self.send_text(text, cx),
            State::Pending => match &mut self.pending_inject {
                Some(pending) => pending.push_str(text),
                None => self.pending_inject = Some(text.to_string()),
            },
            State::Failed(_) => {}
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Find bar owns keys when its input is focused.
        if self.find_bar_focused(window) {
            return;
        }
        // Esc closes find when the terminal body is focused.
        if event.keystroke.key == "escape" && self.find_is_open() {
            self.close_find(window, cx);
            cx.stop_propagation();
            return;
        }
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        self.note_interaction(cx);
        let keystroke = &event.keystroke;
        let handled = terminal.update(cx, |terminal, _| terminal.try_keystroke(keystroke, false));
        if handled {
            cx.stop_propagation();
        }
        cx.notify();
    }

    /// Copy the current selection to the clipboard (no-op without one).
    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state
            && let Some(text) = terminal.read(cx).last_content().selection_text.clone()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Cut: copy the selection (terminal scrollback is not deleted).
    pub fn cut_selection(&self, cx: &mut Context<Self>) {
        self.copy_selection(cx);
    }

    /// Paste the clipboard into the terminal (bracketed-paste aware).
    pub fn paste_clipboard(&self, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state
            && let Some(item) = cx.read_from_clipboard()
            && let Some(text) = terminal_clipboard_text(item)
        {
            terminal.update(cx, |terminal, _| terminal.paste(&text));
        }
    }

    /// Paste shell-quoted paths (Finder drag-drop of images/files).
    pub fn paste_paths(&self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        if self.exited || paths.is_empty() {
            return;
        }
        let State::Ready(terminal) = &self.state else {
            return;
        };
        let text = terminal_paths_text(paths);
        if text.is_empty() {
            return;
        }
        terminal.update(cx, |terminal, _| terminal.paste(&text));
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
        let reporting = terminal.read(cx).mouse_mode(false);
        // Host-select + mouse reporting: cannot use normal mouse_down (reports to
        // the TUI). Shift on simple-click only *extends* and never starts. So:
        // 1–2 clicks → word selection (public API); 3+ → line selection via
        // shift inject (Lines is not the "extend only" path).
        if reporting && !self.mouse_to_app && event.button == MouseButton::Left {
            terminal.update(cx, |terminal, cx| match event.click_count {
                0 => {}
                1 | 2 => terminal.select_word_at_event_position(event),
                _ => {
                    let mut e = event.clone();
                    e.modifiers.shift = true;
                    terminal.mouse_down(&e, cx);
                }
            });
            cx.notify();
            return;
        }
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
        let reporting = terminal.read(cx).mouse_mode(false);
        let mut event = event.clone();
        if inject_shift_for_host_drag(self.mouse_to_app, reporting, event.modifiers.alt) {
            event.modifiers.shift = true;
        }
        if event.pressed_button == Some(MouseButton::Left) {
            let region = terminal.read(cx).last_content().terminal_bounds.bounds;
            terminal.update(cx, |terminal, cx| terminal.mouse_drag(&event, region, cx));
        } else {
            // Hover: zed resolves the link under the cursor only while Cmd is held,
            // populating `last_hovered_word`; grid.rs underlines it from there.
            terminal.update(cx, |terminal, cx| terminal.mouse_move(&event, cx));
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
            let reporting = terminal.read(cx).mouse_mode(false);
            let mut event = event.clone();
            if inject_shift_for_host_drag(self.mouse_to_app, reporting, event.modifiers.alt) {
                event.modifiers.shift = true;
            }
            terminal.update(cx, |terminal, cx| terminal.mouse_up(&event, cx));
            cx.notify();
        }
    }

    /// Hover-only top chrome. Thin hit zone by default; expands into one bar with
    /// status, on-exit, and (when a TUI has mouse reporting) Click in app / Select text.
    fn hover_chrome(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        const HIT_H: f32 = 6.;
        const BAR_H: f32 = 28.;
        let colors = cx.theme().colors().clone();
        let open = self.chrome_hovered;
        let zone = div()
            .id("term-hover-chrome")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(if open { BAR_H } else { HIT_H }))
            .on_hover(cx.listener(|this, hovering, _, cx| {
                if this.chrome_hovered != *hovering {
                    this.chrome_hovered = *hovering;
                    cx.notify();
                }
            }));
        if !open {
            return zone.into_any_element();
        }

        let status = if self.exited {
            "⊘ process exited"
        } else {
            "running"
        };
        let mode = self.auto_close;
        let tip = SharedString::from(format!(
            "When this terminal's process exits: {}. Click to cycle (this tab only; Settings is the default for new tabs).",
            mode.label()
        ));
        let reporting = self.mouse_reporting(cx);
        let to_app = self.mouse_to_app;
        let mouse_modes = reporting.then(|| {
            div()
                .id("term-mouse-policy")
                .flex()
                .items_center()
                .gap_1()
                .px_1()
                .py_0p5()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .bg(colors.elevated_surface_background)
                .child(self.mouse_policy_option(
                    "Click in app",
                    "Left-drag goes to the TUI (default when Grok/etc. want the mouse).",
                    to_app,
                    true,
                    &colors,
                    cx,
                ))
                .child(self.mouse_policy_option(
                    "Select text",
                    "Left-drag selects text to copy. Use this to copy from a TUI.",
                    !to_app,
                    false,
                    &colors,
                    cx,
                ))
        });

        zone.px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .bg(colors.surface_background)
            .border_b_1()
            .border_color(colors.border)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_xs()
                    .text_color(if self.exited {
                        cx.theme().status().ignored
                    } else {
                        colors.text_muted
                    })
                    .child(status),
            )
            .children(mouse_modes)
            .child(
                div()
                    .id("term-auto-close")
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(colors.text)
                    .bg(colors.element_hover)
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_selected))
                    .tooltip(move |_window: &mut Window, cx: &mut App| {
                        cx.new(|_| MouseChipTooltip { text: tip.clone() }).into()
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.cycle_auto_close(cx);
                    }))
                    .child(format!("On exit: {} ▾", mode.short_label())),
            )
            .into_any_element()
    }

    fn mouse_policy_option(
        &self,
        label: &'static str,
        tip: &'static str,
        active: bool,
        set_mouse_to_app: bool,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let tip = SharedString::from(tip);
        div()
            .id(label)
            .px_2()
            .rounded_sm()
            .text_xs()
            .font_weight(if active {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .bg(if active {
                colors.element_selected
            } else {
                gpui::transparent_black()
            })
            .text_color(if active {
                colors.text
            } else {
                colors.text_muted
            })
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .tooltip(move |_window: &mut Window, cx: &mut App| {
                cx.new(|_| MouseChipTooltip { text: tip.clone() }).into()
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.mouse_to_app = set_mouse_to_app;
                cx.notify();
            }))
            .child(label)
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
        let find_bar = self.render_find_bar(&colors, cx);
        let base = div()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .relative()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                this.cut_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                this.copy_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                this.paste_clipboard(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.open_find(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, _, cx| {
                this.find_next(cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, _, cx| {
                this.find_previous(cx);
            }))
            // Finder / external file drag (images and other paths).
            .drag_over::<ExternalPaths>(move |style, _, _, _| {
                style.bg(colors.element_selected)
            })
            .on_drop(cx.listener(
                |this, paths: &ExternalPaths, window, cx| {
                    if this.exited {
                        return;
                    }
                    this.focus.focus(window, cx);
                    this.paste_paths(paths.paths(), cx);
                    this.note_interaction(cx);
                    cx.notify();
                },
            ))
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

        let chrome = self.hover_chrome(cx);
        let menu = self
            .context_menu
            .map(|position| self.render_context_menu(position, cx));
        let tooltip = self
            .hovered_link
            .as_ref()
            .map(|info| self.render_link_tooltip(info, cx));
        match &self.state {
            State::Ready(terminal) => base
                .children(find_bar)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(grid_canvas(
                            terminal.clone(),
                            cx.entity(),
                            self.focus.clone(),
                        ))
                        .child(chrome)
                        .children(tooltip),
                )
                .children(menu),
            State::Pending => base.children(find_bar).child(chrome).children(menu),
            State::Failed(error) => base
                .text_color(cx.theme().colors().text)
                .children(find_bar)
                .child(chrome)
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
        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation());
        // "Open in Browser" only when the right-click landed on an existing file.
        if let Some(path) = self.menu_path.clone() {
            menu_box = menu_box.child(
                context_item("menu-open-browser", "Open in Browser", "", &colors).on_click(
                    cx.listener(move |this, _, _, cx| {
                        cx.open_url(&format!("file://{}", path.display()));
                        this.dismiss_menu(cx);
                    }),
                ),
            );
        }
        let menu_box = menu_box
            .child(
                context_item("menu-cut", "Cut", "⌘X", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cut_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("menu-copy", "Copy", "⌘C", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copy_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("menu-paste", "Paste", "⌘V", &colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.paste_clipboard(cx);
                        this.dismiss_menu(cx);
                    },
                )),
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

fn context_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    colors: &theme::ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    // Prefer element_selected: element_hover often matches elevated_surface.
    let hover = colors.element_selected;
    let muted = colors.text_muted;
    let text = colors.text;
    let row = div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .px_3()
        .py_1()
        .text_sm()
        .text_color(text)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label);
    if shortcut.is_empty() {
        row
    } else {
        row.child(div().text_xs().text_color(muted).child(shortcut))
    }
}

fn grid_canvas(
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    focus: FocusHandle,
) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            let face = xenon_settings::terminal_font(cx);
            let font = grid::terminal_font(&face.family);
            layout(&terminal, &font, face.size, bounds, window, cx)
        },
        move |bounds, grid_layout, window, cx| {
            let size = px(xenon_settings::terminal_font(cx).size);
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
    font_size: f32,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> grid::GridLayout {
    let size = px(font_size);
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar_focused(window) {
            self.append_find_query(text, cx);
            return;
        }
        self.send_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar_focused(window) {
            self.append_find_query(new_text, cx);
            return;
        }
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
