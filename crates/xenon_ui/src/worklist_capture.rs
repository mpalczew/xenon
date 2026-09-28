//! Compact title-and-bullets capture for a workspace worklist. Saves as you type.

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{TypeRole, Typography};
use xenon_editor::item_editor::{ItemEditor, ItemEditorEvent};
use xenon_editor::worklist_file::{ItemDraft, WorkItem};

#[cfg(feature = "visual-tests")]
mod visual;

pub enum CaptureEvent {
    Save {
        item: WorkItem,
    },
    /// ⌘⌫: drop the item and close.
    Discard,
    Close,
}

pub struct WorklistCaptureView {
    editor: Entity<ItemEditor>,
    /// Why the last save failed.
    error: Option<String>,
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
                ItemEditorEvent::Save(item) => CaptureEvent::Save { item: item.clone() },
                ItemEditorEvent::Close => CaptureEvent::Close,
                ItemEditorEvent::Delete => CaptureEvent::Discard,
            });
            cx.notify();
        });
        Self {
            editor,
            error: None,
            _subscription,
        }
    }

    pub fn draft(&self, cx: &App) -> ItemDraft {
        self.editor.read(cx).draft()
    }

    pub fn landed(&mut self, draft: ItemDraft, item: WorkItem, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.landed(draft, item, cx));
    }

    pub fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        cx.notify();
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
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
            .children(self.error.clone().map(|error| {
                div()
                    .mt_2()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.version_control_deleted)
                    .child(error)
            }))
            .id("worklist-capture-panel")
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()));
        div()
            .absolute()
            .inset_0()
            .id("worklist-capture-scrim")
            .on_click(cx.listener(|_, _, _, cx| cx.emit(CaptureEvent::Close)))
            .child(panel)
    }
}
