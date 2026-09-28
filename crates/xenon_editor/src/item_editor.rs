//! The worklist item editor: title, points, and checkbox, shared by the worklist
//! tab and quick capture. It saves as you type by asking its host to land each change.

use crate::worklist_file::entries::Entry;
use crate::worklist_file::{ItemDraft, WorkItem};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
};
use theme::ActiveTheme;
use xenon_design_system::{OutlineEvent, OutlineView, TypeRole, Typography};

#[cfg(feature = "visual-tests")]
mod visual;

pub enum ItemEditorEvent {
    /// The text reads as an item that is not written yet. Land it, then call `landed`.
    Save(WorkItem),
    Close,
    Delete,
}

pub struct ItemEditor {
    outline: Entity<OutlineView>,
    /// None for a note; Some for a task and whether it is done.
    checked: Option<bool>,
    draft: ItemDraft,
    /// The item and done state as last written.
    landed: Option<(WorkItem, bool)>,
    placeholder: Option<SharedString>,
    _subscription: Subscription,
}

impl EventEmitter<ItemEditorEvent> for ItemEditor {}

impl ItemEditor {
    /// A new, unchecked task.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::build("", "", Some(false), cx)
    }

    pub(crate) fn existing(source: &str, entry: &Entry, cx: &mut Context<Self>) -> Self {
        let mut editor = Self::build(&entry.title, &entry.details, entry.checked, cx);
        let done = entry.checked == Some(true);
        editor.draft = ItemDraft::existing(source, entry.range.clone(), done);
        editor.landed = editor.item(cx).ok().map(|item| (item, done));
        editor
    }

    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        self.outline.update(cx, |outline, cx| {
            outline.set_title_placeholder(text.clone(), cx)
        });
        self.placeholder = Some(text);
    }

    pub fn open(&self, cx: &mut App) {
        self.outline
            .update(cx, |outline, cx| outline.open_title(cx));
    }

    pub fn draft(&self) -> ItemDraft {
        self.draft.clone()
    }

    pub fn landed(&mut self, draft: ItemDraft, item: WorkItem, cx: &mut Context<Self>) {
        self.landed = Some((item, self.done()));
        self.draft = draft;
        cx.notify();
    }

    /// True when nothing typed is waiting to be written.
    pub fn settled(&self, cx: &App) -> bool {
        let outline = self.outline.read(cx);
        let typed = !outline.title(cx).trim().is_empty() || !outline.details(cx).trim().is_empty();
        !typed || (self.item(cx).is_ok() && self.pending(cx).is_none())
    }

    /// Why a titled item cannot be written yet.
    fn problem(&self, cx: &App) -> Option<String> {
        let titled = !self.outline.read(cx).title(cx).trim().is_empty();
        self.item(cx)
            .err()
            .filter(|_| titled)
            .map(|error| error.to_string())
    }

    /// Starts a fresh task and returns the draft of the item just edited.
    pub fn reset(&mut self, cx: &mut Context<Self>) -> ItemDraft {
        let draft = std::mem::take(&mut self.draft);
        let placeholder = self.placeholder.take();
        *self = Self::new(cx);
        if let Some(placeholder) = placeholder {
            self.set_placeholder(placeholder, cx);
        }
        cx.notify();
        draft
    }

    fn build(title: &str, details: &str, checked: Option<bool>, cx: &mut Context<Self>) -> Self {
        let outline = cx.new(|cx| OutlineView::new(title, details, checked, cx));
        let _subscription = cx.subscribe(&outline, |this, _, event, cx| match event {
            OutlineEvent::Changed => this.changed(cx),
            OutlineEvent::Checked(checked) => {
                this.checked = Some(*checked);
                this.draft.set_checked(*checked);
                this.changed(cx);
            }
            OutlineEvent::Submit | OutlineEvent::Cancel => cx.emit(ItemEditorEvent::Close),
            OutlineEvent::Delete => cx.emit(ItemEditorEvent::Delete),
        });
        Self {
            outline,
            checked,
            draft: ItemDraft::default(),
            landed: None,
            placeholder: None,
            _subscription,
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.pending(cx) {
            cx.emit(ItemEditorEvent::Save(item));
        }
        cx.notify();
    }

    fn pending(&self, cx: &App) -> Option<WorkItem> {
        let item = self.item(cx).ok()?;
        let unchanged = self
            .landed
            .as_ref()
            .is_some_and(|(landed, done)| *landed == item && *done == self.done());
        (!unchanged).then_some(item)
    }

    fn item(&self, cx: &App) -> anyhow::Result<WorkItem> {
        let outline = self.outline.read(cx);
        WorkItem::new(
            &outline.title(cx),
            &outline.details(cx),
            self.checked.is_some(),
        )
    }

    fn done(&self) -> bool {
        self.checked == Some(true)
    }
}

impl Focusable for ItemEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.outline.read(cx).focus_handle(cx)
    }
}

impl Render for ItemEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let problem = self.problem(cx).map(|problem| {
            div()
                .mt_2()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(cx.theme().colors().version_control_deleted)
                .child(problem)
        });
        div().child(self.outline.clone()).children(problem)
    }
}
