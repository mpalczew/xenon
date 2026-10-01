//! Inline section naming: rename a header, or name a section that does not exist yet.

use super::{Cursor, EditorView};
use crate::worklist_file::entries::parse;
use crate::worklist_file::{Target, delete_empty_section, insert_section, rename_section};
use gpui::{AppContext, Context, Entity, Subscription};
use xenon_design_system::{TextInputAppearance, TextInputConfig, TextInputEvent, TextInputView};

pub(in crate::view) enum SectionEditKind {
    Rename(Target),
    /// A new section after this one's items (the end of the file when None).
    New(Option<Target>),
}

pub(in crate::view) struct SectionEdit {
    pub(super) kind: SectionEditKind,
    pub(super) input: Entity<TextInputView>,
    _submit: Subscription,
    pub(super) blur: Option<Subscription>,
}

impl EditorView {
    pub(super) fn worklist_start_rename(&mut self, index: usize, cx: &mut Context<Self>) {
        let parsed = parse(&self.text());
        let Some(section) = parsed.sections.get(index) else {
            return;
        };
        let target = Target::of(&parsed.sections, Some(index));
        self.worklist_begin_section_edit(SectionEditKind::Rename(target), &section.title, cx);
    }

    pub(super) fn worklist_start_new_section(&mut self, cx: &mut Context<Self>) {
        let parsed = parse(&self.text());
        let section = match self.worklist_cursor {
            Cursor::Header(index) => Some(index),
            Cursor::Item(index) => parsed.entries.get(index).and_then(|entry| entry.section),
        };
        let has_selection = !parsed.entries.is_empty() || !parsed.sections.is_empty();
        let after = has_selection.then(|| Target::of(&parsed.sections, section));
        self.worklist_begin_section_edit(SectionEditKind::New(after), "", cx);
    }

    fn worklist_begin_section_edit(
        &mut self,
        kind: SectionEditKind,
        initial: &str,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let config = TextInputConfig::single_line("Section name")
                .appearance(TextInputAppearance::Inline)
                .min_height(gpui::px(20.));
            let mut input = TextInputView::new(config, cx);
            input.set_text(initial, cx);
            input.open(cx);
            input
        });
        let submit = cx.subscribe(&input, |this, _, event, cx| match event {
            TextInputEvent::Submit(text) => {
                this.worklist_finish_section_edit(Some(text.clone()), cx)
            }
            TextInputEvent::Cancel => this.worklist_finish_section_edit(None, cx),
            _ => {}
        });
        self.worklist_error = None;
        self.worklist_section_edit = Some(SectionEdit {
            kind,
            input,
            _submit: submit,
            blur: None,
        });
        cx.notify();
    }

    /// Closes the field; `text` is None when the user cancelled.
    pub(super) fn worklist_finish_section_edit(
        &mut self,
        text: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.worklist_section_edit.take() else {
            return;
        };
        self.worklist_refocus = true;
        if let Some(text) = text {
            match edit.kind {
                SectionEditKind::Rename(target) => self.worklist_rename(&target, &text, cx),
                SectionEditKind::New(after) => self.worklist_create_section(after, &text, cx),
            }
        }
        cx.notify();
    }

    fn worklist_rename(&mut self, target: &Target, text: &str, cx: &mut Context<Self>) {
        let source = self.text();
        let title = text.trim();
        if title == target.label() {
            return;
        }
        let edit = if title.is_empty() {
            delete_empty_section(&source, target)
        } else {
            rename_section(&source, target, title)
        };
        match edit {
            Ok(edit) => {
                if self.worklist_mutate(edit.range, &edit.text, cx)
                    && self.worklist_folded.remove(target.label())
                    && !title.is_empty()
                {
                    self.worklist_folded.insert(title.to_owned());
                }
            }
            Err(error) => self.worklist_error = Some(error.to_string()),
        }
    }

    fn worklist_create_section(
        &mut self,
        after: Option<Target>,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let title = text.trim();
        if title.is_empty() {
            return;
        }
        let edit = match insert_section(&self.text(), after.as_ref(), title) {
            Ok(edit) => edit,
            Err(error) => {
                self.worklist_error = Some(error.to_string());
                return;
            }
        };
        let at = edit.range.start;
        if self.worklist_mutate(edit.range, &edit.text, cx) {
            let sections = parse(&self.text()).sections;
            let created = sections
                .iter()
                .position(|s| s.start >= at && s.title == title);
            if let Some(index) = created {
                self.worklist_cursor = Cursor::Header(index);
            }
        }
    }
}
