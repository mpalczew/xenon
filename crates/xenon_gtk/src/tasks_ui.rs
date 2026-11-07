//! What a task run shows: the Run Task picker, the `inputs` prompts, and the
//! folderOpen consent card. The picker and prompts are the palette shell
//! (`palette.rs`); the rules and texts are `xenon_core::tasks`.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use xenon_core::tasks::{self as shared, QuestionKind, TaskQuestion, TaskRow};

use crate::app::App;
use crate::palette::{Handlers, Options, PaletteRow};

/// Indexes of `labels` that match `query`, best first, with the matched letters.
fn fuzzy(labels: &[String], query: &str) -> Vec<(usize, Vec<u32>)> {
    if query.is_empty() {
        return (0..labels.len()).map(|i| (i, Vec::new())).collect();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize)> = labels
        .iter()
        .enumerate()
        .filter_map(|(i, label)| {
            pattern
                .score(Utf32Str::new(label, &mut buf), &mut matcher)
                .map(|score| (score, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .map(|(_, i)| (i, crate::picker::hits(&labels[i], query)))
        .collect()
}

impl App {
    /// Ctrl+Shift+R: fuzzy-pick a task of the workspace. Enter runs it per its
    /// presentation; Ctrl+Enter runs it in a new tab of its own.
    pub(crate) fn open_task_picker(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            self.no_workspace_toast();
            return;
        };
        let (rows, error) = match self.load_workspace_tasks(ws) {
            Ok(loaded) => {
                let rows = shared::task_rows(&loaded.set, &loaded.folder);
                let error = rows.is_empty().then(|| shared::NO_TASKS.to_string());
                (rows, error)
            }
            Err(error) => (Vec::new(), Some(error)),
        };
        let labels: Vec<String> = rows.iter().map(|r| r.label.clone()).collect();
        let rows: Rc<Vec<TaskRow>> = Rc::new(rows);
        let results: Rc<RefCell<Vec<usize>>> = Rc::default();
        let failed = error.is_some();
        let weak_q = Rc::downgrade(self);
        let weak = Rc::downgrade(self);
        let rows_q = rows.clone();
        let results_q = results.clone();
        let accept: Rc<dyn Fn(usize, bool)> = Rc::new(move |row, new_tab| {
            let Some(app) = weak.upgrade() else { return };
            let chosen = results
                .borrow()
                .get(row)
                .and_then(|&i| rows.get(i))
                .cloned();
            let Some(chosen) = chosen else { return };
            if let Some(reason) = &chosen.unsupported {
                app.toast_failed(shared::CANT_RUN, &format!("{}: {reason}", chosen.label));
            } else {
                app.launcher.close();
                app.run_task_checked(ws, chosen.label, new_tab);
            }
        });
        let (accept_plain, accept_alt) = (accept.clone(), accept);
        self.launcher.open(
            &self.win,
            Handlers {
                placeholder: error.unwrap_or_else(|| shared::PICKER_PLACEHOLDER.to_string()),
                hint: xenon_keymap::display_text(shared::PICKER_HINT),
                options: Options {
                    on_accept_alt: Some(Box::new(move |row| accept_alt(row, true))),
                    ..Options::default()
                },
                on_query: Box::new(move |query| {
                    let Some(app) = weak_q.upgrade() else { return };
                    let found = fuzzy(&labels, query);
                    let palette_rows = found
                        .iter()
                        .map(|(i, hits)| {
                            let row = &rows_q[*i];
                            PaletteRow {
                                title: row.label.clone(),
                                hits: hits.clone(),
                                detail: Some(row.subtitle()),
                                badge: row.chip(),
                                enabled: true,
                            }
                        })
                        .collect();
                    *results_q.borrow_mut() = found.into_iter().map(|(i, _)| i).collect();
                    let empty = shared::picker_empty_message(failed, query.is_empty());
                    app.launcher.set_rows(palette_rows, empty);
                }),
                on_accept: Box::new(move |row| accept_plain(row, false)),
                on_complete: None,
            },
        );
    }

    /// Ask one `inputs` entry: a text field (hidden for a password) or a
    /// filtered pick list. Enter answers, Escape cancels the whole run.
    pub(crate) fn show_input_prompt(self: &Rc<Self>, question: TaskQuestion) {
        let (weak_a, weak_d, weak_q) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        let mut options = Options {
            prompt: Some(question.prompt_text().to_string()),
            on_dismiss: Some(Box::new(move || {
                if let Some(app) = weak_d.upgrade() {
                    app.answer_task_input(None);
                }
            })),
            ..Options::default()
        };
        let results: Rc<RefCell<Vec<usize>>> = Rc::default();
        let choices = match &question.kind {
            QuestionKind::Pick { options } => options.clone(),
            QuestionKind::Text { password } => {
                options.initial = question.default.clone().unwrap_or_default();
                options.masked = *password;
                options.free_text = true;
                Vec::new()
            }
        };
        let is_pick = matches!(question.kind, QuestionKind::Pick { .. });
        let labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
        let default = question.default.clone();
        let (choices_a, results_a, results_q) = (choices.clone(), results.clone(), results);
        self.launcher.open(
            &self.win,
            Handlers {
                placeholder: question.placeholder().to_string(),
                hint: question.hint().to_string(),
                options,
                on_query: Box::new(move |query| {
                    let Some(app) = weak_q.upgrade() else { return };
                    if !is_pick {
                        app.launcher.set_rows(Vec::new(), "");
                        return;
                    }
                    let found = fuzzy(&labels, query);
                    let rows = found
                        .iter()
                        .map(|(i, hits)| {
                            let choice = &choices[*i];
                            PaletteRow {
                                title: choice.label.clone(),
                                hits: hits.clone(),
                                detail: (choice.value != choice.label)
                                    .then(|| choice.value.clone()),
                                badge: None,
                                enabled: true,
                            }
                        })
                        .collect();
                    // Opening selects the default choice, as the field is empty.
                    let at = default.as_ref().filter(|_| query.is_empty()).and_then(|d| {
                        found
                            .iter()
                            .position(|(i, _)| choices[*i].value == *d || choices[*i].label == *d)
                    });
                    *results_q.borrow_mut() = found.into_iter().map(|(i, _)| i).collect();
                    app.launcher.set_rows(rows, shared::NO_MATCHING_CHOICES);
                    if let Some(at) = at {
                        app.launcher.select(at as u32);
                    }
                }),
                on_accept: Box::new(move |row| {
                    let Some(app) = weak_a.upgrade() else { return };
                    let value = if is_pick {
                        results_a
                            .borrow()
                            .get(row)
                            .and_then(|&i| choices_a.get(i))
                            .map(|c| c.value.clone())
                    } else {
                        Some(app.launcher.text())
                    };
                    if let Some(value) = value {
                        app.launcher.close();
                        app.answer_task_input(Some(value));
                    }
                }),
                on_complete: None,
            },
        );
    }
}

/// The folderOpen consent card: a corner card that never takes focus or
/// Escape. Its two keys work from anywhere while it shows.
pub(crate) struct TrustCard {
    pub root: gtk::Box,
    title: gtk::Label,
    list: gtk::Box,
}

impl TrustCard {
    pub(crate) fn new(app: Weak<App>) -> Self {
        let title = gtk::Label::new(None);
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.add_css_class("heading");
        let detail = gtk::Label::new(Some(shared::TRUST_DETAIL));
        detail.set_xalign(0.0);
        detail.set_wrap(true);
        detail.add_css_class("dim-label");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&list));
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_max_content_height(220);
        scroller.set_propagate_natural_height(true);
        scroller.set_focusable(false);
        let button = |text: &str, allow: bool| {
            let b = gtk::Button::with_label(text);
            b.set_focusable(false);
            if allow {
                b.add_css_class("suggested-action");
            }
            let app = app.clone();
            b.connect_clicked(move |_| {
                if let Some(app) = app.upgrade() {
                    app.answer_folder_tasks(allow);
                }
            });
            b
        };
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        buttons.append(&button(shared::TRUST_DENY, false));
        buttons.append(&button(shared::TRUST_ALLOW, true));
        let hint = gtk::Label::new(Some(&xenon_keymap::display_text(shared::TRUST_HINT)));
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
        inner.set_margin_top(12);
        inner.set_margin_bottom(12);
        inner.set_margin_start(12);
        inner.set_margin_end(12);
        inner.append(&title);
        inner.append(&detail);
        inner.append(&scroller);
        inner.append(&buttons);
        inner.append(&hint);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("card");
        root.add_css_class("background");
        root.set_size_request(460, -1);
        root.set_halign(gtk::Align::End);
        root.set_valign(gtk::Align::End);
        root.set_margin_end(16);
        root.set_margin_bottom(16);
        root.set_focusable(false);
        root.append(&inner);
        root.set_visible(false);
        Self { root, title, list }
    }

    pub(crate) fn show(&self, title: &str, lines: &[(String, String, &str)]) {
        self.title.set_text(title);
        while let Some(old) = self.list.first_child() {
            self.list.remove(&old);
        }
        for (label, command, why) in lines {
            let name = gtk::Label::new(Some(&format!("{label} \u{b7} {why}")));
            name.set_xalign(0.0);
            name.add_css_class("dim-label");
            let line = gtk::Label::new(Some(&format!("$ {command}")));
            line.set_xalign(0.0);
            line.set_wrap(true);
            line.set_selectable(false);
            line.add_css_class("monospace");
            let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
            row.append(&name);
            row.append(&line);
            self.list.append(&row);
        }
        self.root.set_visible(true);
    }

    pub(crate) fn hide(&self) {
        self.root.set_visible(false);
    }
}
