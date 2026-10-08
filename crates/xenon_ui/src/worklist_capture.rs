//! Compact title-and-bullets capture for a workspace worklist. Saves as you type.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{ActionButton, TypeRole, Typography, action_button};
use xenon_editor::item_editor::{ItemEditor, ItemEditorEvent};
use xenon_editor::worklist_file::{ItemDraft, Target, WorkItem};

#[cfg(feature = "visual-tests")]
mod visual;

pub enum CaptureEvent {
    Save {
        item: WorkItem,
        target: Target,
    },
    /// ⌘⌫: drop the item and close.
    Discard,
    Close,
    /// ⌥↑/⌥↓: the section new text will land in.
    Section(Target),
}

pub struct WorklistCaptureView {
    editor: Entity<ItemEditor>,
    /// Why the last save failed.
    error: Option<String>,
    /// Where the item files: sections in file order, perhaps the top first.
    targets: Vec<Target>,
    selected: usize,
    _subscription: Subscription,
}

impl EventEmitter<CaptureEvent> for WorklistCaptureView {}

impl WorklistCaptureView {
    pub fn new(workspace_name: String, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = ItemEditor::new(cx);
            editor.set_placeholder(format!("Task for {workspace_name}"), cx);
            editor
        });
        let _subscription = cx.subscribe(&editor, |this, _, event, cx| {
            this.error = None;
            cx.emit(match event {
                ItemEditorEvent::Save(item) => CaptureEvent::Save {
                    item: item.clone(),
                    target: this.target(),
                },
                ItemEditorEvent::Close => CaptureEvent::Close,
                ItemEditorEvent::Delete => CaptureEvent::Discard,
            });
            cx.notify();
        });
        Self {
            editor,
            error: None,
            targets: Vec::new(),
            selected: 0,
            _subscription,
        }
    }

    fn target(&self) -> Target {
        self.targets.get(self.selected).cloned().unwrap_or_default()
    }

    fn has_sections(&self) -> bool {
        self.targets.iter().any(Target::is_section)
    }

    /// Once saved, the item stays where it landed; moving it is a Markdown edit.
    fn locked(&self, cx: &App) -> bool {
        self.editor.read(cx).draft().is_saved()
    }

    fn cycle(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.targets.len() as isize;
        if count < 2 || self.locked(cx) {
            return;
        }
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
        cx.emit(CaptureEvent::Section(self.target()));
        cx.notify();
    }

    pub fn draft(&self, cx: &App) -> ItemDraft {
        let draft = self.editor.read(cx).draft();
        let mut draft = draft;
        if !draft.is_saved() {
            draft.set_target(self.target());
        }
        draft
    }

    pub fn landed(&mut self, draft: ItemDraft, item: WorkItem, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.landed(draft, item, cx));
    }

    pub fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        cx.notify();
    }

    /// Shows the capture, offering `targets` and picking `preferred` when it is
    /// among them, else the first.
    pub fn open(
        &mut self,
        targets: Vec<Target>,
        preferred: Option<&Target>,
        cx: &mut Context<Self>,
    ) {
        self.selected = preferred
            .and_then(|want| targets.iter().position(|t| t.label() == want.label()))
            .unwrap_or(0);
        self.targets = targets;
        self.editor.update(cx, |editor, cx| editor.open(cx));
        cx.notify();
    }

    /// Starts fresh when everything typed is saved and returns the saved item.
    /// Keeps unsaved text for the next open.
    pub fn finish(&mut self, cx: &mut Context<Self>) -> Option<ItemDraft> {
        if !self.editor.read(cx).settled(cx) {
            return None;
        }
        let draft = self.reset(cx);
        draft.is_saved().then_some(draft)
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) -> ItemDraft {
        self.error = None;
        self.editor.update(cx, |editor, cx| editor.reset(cx))
    }
}

impl Focusable for WorklistCaptureView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }
}

impl Render for WorklistCaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let panel = div()
            .absolute()
            .top(px(48.))
            .right(px(12.))
            .w(px(430.))
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .text_color(colors.text)
            .shadow_lg()
            .child(self.editor.clone())
            .children(self.has_sections().then(|| self.render_section_chip(cx)))
            .children(self.error.clone().map(|error| {
                div()
                    .mt_2()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.version_control_deleted)
                    .child(error)
            }))
            .id("worklist-capture-panel")
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.alt {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "up" => this.cycle(-1, cx),
                    "down" => this.cycle(1, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()));
        div()
            .absolute()
            .inset_0()
            .id("worklist-capture-scrim")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(CaptureEvent::Close)))
            .child(panel)
    }
}

impl WorklistCaptureView {
    /// "Into ‹ Section ›": cycles with ⌥↑/⌥↓ or the arrows.
    fn render_section_chip(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors().clone();
        let locked = self.locked(cx);
        let step = |id: &'static str, glyph: &'static str, delta: isize, cx: &mut Context<Self>| {
            action_button(
                id,
                ActionButton::icon(glyph),
                cx,
                cx.listener(move |this, _, _, cx| this.cycle(delta, cx)),
            )
            .w(px(22.))
            .min_h(px(22.))
        };
        let chip = div()
            .flex()
            .items_center()
            .rounded_full()
            .border_1()
            .border_color(colors.border)
            .bg(colors.element_background)
            .pl_1()
            .pr_1()
            .text_color(colors.text)
            .opacity(if locked { 0.6 } else { 1. })
            .child(step("worklist-section-prev", "‹", -1, cx))
            .child(
                div()
                    .id("worklist-capture-section")
                    .min_w(px(84.))
                    .px_1()
                    .flex()
                    .justify_center()
                    .child(self.target().label().to_owned()),
            )
            .child(step("worklist-section-next", "›", 1, cx));
        let hint = if locked {
            "Saved here".to_owned()
        } else {
            xenon_design_system::shortcut_text("⌥↑ ⌥↓ section")
        };
        div()
            .mt_3()
            .flex()
            .items_center()
            .gap_2()
            .type_role(TypeRole::ControlLabel, cx)
            .child("Into")
            .child(chip)
            .child(div().flex_1())
            .child(hint)
            .into_any_element()
    }
}
