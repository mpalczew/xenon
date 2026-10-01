//! Source-preserving worklist presentation over the editor's Markdown buffer.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use theme::ActiveTheme;
use xenon_design_system::{
    ActionButton, AllClear, CheckboxState, TypeRole, Typography, action_button, all_clear,
    checkbox, key_chip,
};

use super::{Content, EditorView};
mod capture;
mod edit;
mod edit_view;
mod header;
mod keys;
mod rows;
mod section_edit;
use crate::worklist_file::Target;
use crate::worklist_file::entries::{Entry, Parsed, parse};
use header::HeaderState;
pub(in crate::view) use rows::Cursor;
pub(in crate::view) use section_edit::SectionEdit;
use xenon_design_system::{BulletLine, bullet_list, selectable_row};

/// What one paint of the list shares between its regions.
struct Frame<'a> {
    parsed: &'a Parsed,
    cursor: Cursor,
    /// The region an Add in progress files into.
    add_region: Option<Option<usize>>,
}

struct RowState {
    index: usize,
    entry: Entry,
    selected: bool,
}

impl EditorView {
    pub(super) fn worklist_needs_creation(&self) -> bool {
        matches!(&self.content, Content::Text(buffer) if buffer.worklist_needs_creation())
    }

    pub(super) fn render_worklist_raw_header(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        action_button(
            "worklist-back-to-list",
            ActionButton::quiet("← Worklist    Markdown"),
            cx,
            cx.listener(|this, _, _, cx| {
                if !this.is_dirty() {
                    this.worklist_raw = false;
                    cx.notify();
                } else {
                    let toast = xenon_design_system::Toast::info("✋", "Save or undo first")
                        .detail("Markdown edits need to land before the list view.");
                    xenon_design_system::show_toast(toast, cx);
                }
            }),
        )
        .justify_start()
        .w_full()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(colors.border)
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_edit(&mut self, cx: &mut Context<Self>) {
        self.worklist_start_edit(0, cx);
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_capture(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.worklist_start_add(Target::Top, cx);
    }

    /// Select a section header and optionally fold it, as the keyboard would.
    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_header(&mut self, index: usize, fold: bool, cx: &mut Context<Self>) {
        self.worklist_cursor = Cursor::Header(index);
        if fold {
            self.worklist_fold(index, Some(true), cx);
        }
        cx.notify();
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_rename(&mut self, index: usize, cx: &mut Context<Self>) {
        self.worklist_cursor = Cursor::Header(index);
        self.worklist_start_rename(index, cx);
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_add_to(&mut self, index: usize, cx: &mut Context<Self>) {
        let target = Target::of(&parse(&self.text()).sections, Some(index));
        self.worklist_start_add(target, cx);
    }

    #[cfg(feature = "visual-tests")]
    pub fn visual_worklist_markdown(&mut self, cx: &mut Context<Self>) {
        self.worklist_raw = true;
        cx.notify();
    }

    pub(super) fn render_worklist(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        if std::mem::take(&mut self.worklist_refocus) {
            self.focus.focus(window, cx);
        }
        self.watch_section_edit_blur(window, cx);
        let colors = cx.theme().colors().clone();
        let body = self.render_worklist_body(&colors, cx);
        div()
            .id("worklist-view")
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .children(self.disk_alert_bar(cx))
            .children(self.worklist_error.clone().map(|error| {
                div()
                    .px_4()
                    .py_2()
                    .bg(colors.element_background)
                    .text_color(colors.version_control_deleted)
                    .child(error)
            }))
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().type_role(TypeRole::ScreenTitle, cx).child("Worklist"))
                            .child(
                                div()
                                    .type_role(TypeRole::Code, cx)
                                    .text_color(colors.text_muted)
                                    .child(".xenon/worklist.md"),
                            ),
                    )
                    .child(action_button(
                        "worklist-markdown",
                        ActionButton::secondary("Markdown"),
                        cx,
                        cx.listener(|this, _, _, cx| {
                            this.worklist_raw = true;
                            cx.notify();
                        }),
                    )),
            )
            .child(body)
    }

    fn render_worklist_body(
        &mut self,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let source = self.text();
        let parsed = parse(&source);
        let rows = rows::visible(&parsed.sections, &parsed.entries, &self.worklist_folded);
        let cursor = rows::settle(&rows, self.worklist_cursor, &parsed.entries);
        let pending = self
            .worklist_edit
            .as_ref()
            .is_some_and(|edit| !edit.read(cx).draft().is_saved());
        if parsed.entries.is_empty()
            && parsed.sections.is_empty()
            && !pending
            && self.worklist_section_edit.is_none()
        {
            return self.render_worklist_all_clear(cx);
        }
        let add_region = pending
            .then(|| self.worklist_add_target.resolve(&parsed.sections).ok())
            .flatten();
        let frame = Frame {
            parsed: &parsed,
            cursor,
            add_region,
        };
        let mut children = vec![self.render_worklist_actions(cursor, &parsed, cx)];
        let regions = std::iter::once(None).chain((0..parsed.sections.len()).map(Some));
        for region in regions {
            children.extend(self.render_worklist_region(region, &frame, colors, cx));
        }
        div()
            .id("worklist-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .children(children)
            .into_any_element()
    }

    /// "+ Add an item" files into the selected row's section; "+ Section" starts a new one.
    fn render_worklist_actions(
        &mut self,
        cursor: Cursor,
        parsed: &Parsed,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let section = match cursor {
            Cursor::Header(index) => Some(index),
            Cursor::Item(index) => parsed.entries.get(index).and_then(|entry| entry.section),
        };
        let target = Target::of(&parsed.sections, section);
        div()
            .flex()
            .gap_4()
            .mb_3()
            .child(action_button(
                "worklist-add",
                ActionButton::quiet("+ Add an item"),
                cx,
                cx.listener(move |this, _, _, cx| this.worklist_start_add(target.clone(), cx)),
            ))
            .child(action_button(
                "worklist-add-section",
                ActionButton::quiet("+ Section"),
                cx,
                cx.listener(|this, _, _, cx| this.worklist_start_new_section(cx)),
            ))
            .into_any_element()
    }

    /// One section (None: the items above the first): header, items, then any open editor.
    fn render_worklist_region(
        &mut self,
        region: Option<usize>,
        frame: &Frame<'_>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let Frame {
            parsed,
            cursor,
            add_region,
        } = *frame;
        let mut out = Vec::new();
        let editing = self
            .worklist_edit
            .as_ref()
            .is_some_and(|edit| edit.read(cx).draft().is_saved());
        let in_region = |entry: &Entry| entry.section == region;
        let items = parsed
            .entries
            .iter()
            .filter(|entry| in_region(entry))
            .count();
        let mut folded = false;
        if let Some(index) = region {
            let section = &parsed.sections[index];
            folded = self.worklist_folded.contains(&section.title);
            let open = parsed
                .entries
                .iter()
                .filter(|entry| in_region(entry) && entry.checked == Some(false))
                .count();
            let renaming = matches!(
                &self.worklist_section_edit,
                Some(edit) if matches!(&edit.kind, section_edit::SectionEditKind::Rename(Target::Section { index: i, .. }) if *i == index)
            );
            let state = HeaderState {
                index,
                title: section.title.clone(),
                open,
                items,
                selected: cursor == Cursor::Header(index),
                folded,
                renaming,
            };
            out.push(self.render_worklist_header(state, colors, cx));
        }
        let adding_here = add_region == Some(region);
        if !folded {
            for (index, entry) in parsed
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| in_region(e))
            {
                let selected = cursor == Cursor::Item(index);
                out.push(if editing && selected {
                    self.render_worklist_edit(colors, cx).into_any_element()
                } else {
                    let row = RowState {
                        index,
                        entry: entry.clone(),
                        selected,
                    };
                    self.render_worklist_row(row, colors, cx).into_any_element()
                });
            }
            if items == 0 && region.is_some() && !adding_here {
                out.push(self.render_worklist_empty_note(colors, cx));
            }
        }
        if adding_here {
            out.push(self.render_worklist_edit(colors, cx).into_any_element());
        }
        if self.new_section_follows(region, parsed) {
            out.extend(self.render_worklist_new_header(colors, cx));
        }
        out
    }

    /// True when the section being named goes right after `region`'s items.
    fn new_section_follows(&self, region: Option<usize>, parsed: &Parsed) -> bool {
        let Some(edit) = &self.worklist_section_edit else {
            return false;
        };
        let section_edit::SectionEditKind::New(after) = &edit.kind else {
            return false;
        };
        let end = parsed.sections.len().checked_sub(1);
        match after {
            None => region == end,
            Some(target) => target.resolve(&parsed.sections).ok() == Some(region),
        }
    }

    /// Naming ends when the field loses focus, as clicking elsewhere would expect.
    fn watch_section_edit_blur(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let Some(edit) = &mut self.worklist_section_edit else {
            return;
        };
        if edit.blur.is_some() {
            return;
        }
        let input = edit.input.clone();
        let focus = input.read(cx).focus_handle();
        edit.blur = Some(cx.on_blur(&focus, window, move |this, _, cx| {
            let text = input.read(cx).text().to_owned();
            this.worklist_finish_section_edit(Some(text), cx);
        }));
    }

    fn render_worklist_all_clear(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let copy = AllClear {
            title: "All clear.".into(),
            detail: "Nothing's waiting on you here.".into(),
        };
        let hover = cx.theme().colors().element_hover;
        let hints = div()
            .flex()
            .gap_2p5()
            .mt_2()
            .child(
                key_chip("worklist-add", "↵", "add an item", cx)
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.worklist_start_add(Target::Top, cx)),
                    ),
            )
            .child(key_chip(
                "worklist-capture-hint",
                crate::worklist_file::CAPTURE_KEYS,
                "from anywhere",
                cx,
            ));
        div()
            .flex_1()
            .min_h_0()
            .child(all_clear("worklist-all-clear", copy, cx).child(hints))
            .into_any_element()
    }

    fn render_worklist_row(
        &mut self,
        row: RowState,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let RowState {
            index,
            entry,
            selected,
        } = row;
        let checked = entry.checked;
        let editable = entry.editable;
        let title = entry.title.clone();
        let lines: Vec<_> = xenon_design_system::points_from_details(&entry.details)
            .into_iter()
            .map(|point| BulletLine {
                depth: point.depth,
                text: point.text.into(),
            })
            .collect();
        selectable_row(format!("worklist-row-{index}"), selected, colors)
            .items_start()
            .mb_1()
            // Whole row, not just the text: the row padding looks and hovers clickable.
            .on_click(cx.listener(move |this, _, _, cx| {
                this.worklist_cursor = Cursor::Item(index);
                if editable {
                    this.worklist_start_edit(index, cx);
                }
            }))
            .child(if let Some(done) = checked {
                checkbox(
                    format!("worklist-check-{index}"),
                    CheckboxState {
                        checked: done,
                        disabled: false,
                    },
                    format!("Complete {title}"),
                    cx,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.worklist_cursor = Cursor::Item(index);
                        this.worklist_toggle(index, cx);
                    }),
                )
                .into_any_element()
            } else {
                div().w(px(18.)).child("•").into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(if checked == Some(true) {
                        colors.text_muted
                    } else {
                        colors.text
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child({
                                let title_line = div().type_role(TypeRole::ListPrimary, cx);
                                let title_line = if lines.is_empty() {
                                    title_line
                                } else {
                                    title_line.mb_3()
                                };
                                title_line.child(title)
                            })
                            .child(bullet_list(lines, cx)),
                    ),
            )
            .child(action_button(
                format!("worklist-edit-{index}"),
                ActionButton::quiet("Edit").disabled(!editable),
                cx,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.worklist_cursor = Cursor::Item(index);
                    this.worklist_start_edit(index, cx);
                }),
            ))
    }
}
