//! The worklist item editor: title, points, and checkbox, shared by the worklist
//! tab and quick capture. It saves as you type by asking its host to land each change.

use crate::worklist_file::entries::Entry;
use crate::worklist_file::{ItemDraft, TITLE_LIMIT, WorkItem, title_length};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
};
use theme::ActiveTheme;
use xenon_design_system::{OutlineEvent, OutlineView, TypeRole, Typography};

#[cfg(feature = "visual-tests")]
mod visual;

/// Characters left at which the countdown appears.
const NEAR_LIMIT: isize = 12;

struct LengthHint {
    left: isize,
    note: Option<&'static str>,
}

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
        let outline = cx.new(|cx| {
            let outline = OutlineView::new(title, details, checked, cx);
            outline.limit_title(TITLE_LIMIT, cx);
            outline
        });
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

    /// Countdown near the soft title limit, and a note once past it.
    fn length_hint(&self, cx: &App) -> Option<LengthHint> {
        let left =
            TITLE_LIMIT as isize - title_length(self.outline.read(cx).title(cx).trim()) as isize;
        if left > NEAR_LIMIT {
            return None;
        }
        let note = (left < 0).then(|| {
            if self.draft.is_saved() && self.settled(cx) {
                "Saved · long titles wrap in the list"
            } else {
                "Long titles wrap in the list"
            }
        });
        Some(LengthHint { left, note })
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
        let hint = self.length_hint(cx).map(|hint| {
            let muted = cx.theme().colors().text_muted;
            let count = if hint.left < 0 {
                cx.theme().status().warning
            } else {
                muted
            };
            div()
                .mt_2()
                .flex()
                .justify_between()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(muted)
                .child(div().children(hint.note))
                .child(div().text_color(count).child(hint.left.to_string()))
        });
        div().child(self.outline.clone()).children(hint)
    }
}
