//! The worklist: quick capture into `.xenon/worklist.md` and "Open Worklist".
//!
//! The file model (parse, edit, sections) is `xenon_store::worklist`. Opening
//! shows the file in the list view (`worklist_list.rs`) or, by choice, the
//! Markdown writing mode; capture is a stock `adw::Dialog` that saves as you
//! type.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use xenon_core::WorkspaceId;
use xenon_store::worklist::entries::Entry;
use xenon_store::worklist::{
    Edit, ItemDraft, Target, WorklistFile, capture_targets, read_source, write_draft,
};

use crate::app::App;

mod capture;
mod fields;
use capture::Capture;

/// Capture state that outlives one dialog.
#[derive(Default)]
pub(crate) struct Worklist {
    capture: RefCell<Option<Rc<Capture>>>,
    /// The section new text last went into, per workspace.
    last_section: RefCell<HashMap<WorkspaceId, Target>>,
    /// The item the showing toast can take back, and which toast that is.
    undo: RefCell<Option<(WorkspaceId, ItemDraft)>>,
    toast_serial: Cell<u64>,
}

impl App {
    fn workspace_for_worklist(&self) -> Option<(WorkspaceId, std::path::PathBuf, String)> {
        let model = self.model.borrow();
        let ws = model.active()?;
        let rec = model.rec(ws)?;
        Some((ws, rec.root.clone(), rec.name.clone()))
    }

    /// Open Worklist: `.xenon/worklist.md` in the list view, made when missing.
    pub(crate) fn open_worklist(self: &Rc<Self>) {
        let Some((ws, root, _)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let capture = self.worklist.capture.borrow().clone();
        if let Some(capture) = capture {
            capture.dialog.close();
        }
        let made = WorklistFile::new(&root).and_then(|file| match file.read()? {
            Some(_) => Ok(()),
            None => file.write(None, b"# Worklist\n"),
        });
        if let Err(error) = made {
            self.toast_failed("Couldn\u{2019}t open the worklist", &error.to_string());
            return;
        }
        self.open_editor(ws, &root.join(".xenon/worklist.md"));
    }

    /// Add to Worklist: the capture dialog; the same key again closes it.
    pub(crate) fn capture_worklist(self: &Rc<Self>) {
        self.open_capture(None, None);
    }

    /// Add to Worklist with `section` chosen (from the list view's + button).
    pub(crate) fn capture_worklist_into(self: &Rc<Self>, section: Target) {
        self.open_capture(Some(section), None);
    }

    /// The capture form prefilled with `entry` of `source`, with a Delete button.
    pub(crate) fn edit_worklist_item(self: &Rc<Self>, entry: Entry, source: String) {
        self.open_capture(None, Some((entry, source)));
    }

    fn open_capture(self: &Rc<Self>, section: Option<Target>, edit: Option<(Entry, String)>) {
        let open = self.worklist.capture.borrow().clone();
        if let Some(open) = open {
            open.dialog.close();
            return;
        }
        let Some((ws, root, name)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let targets = capture_targets(&read_source(&root));
        let preferred = section.or_else(|| self.worklist.last_section.borrow().get(&ws).cloned());
        let selected = preferred
            .and_then(|want| targets.iter().position(|t| t.label() == want.label()))
            .unwrap_or(0);
        let capture = Capture::build(self, (ws, root), &name, (targets, selected), edit);
        *self.worklist.capture.borrow_mut() = Some(capture.clone());
        capture.dialog.present(Some(&self.win));
        capture.title.grab_focus();
    }

    /// Take back the capture the toast offers.
    pub(crate) fn undo_capture(self: &Rc<Self>) {
        let Some((ws, mut draft)) = self.worklist.undo.borrow_mut().take() else {
            return;
        };
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        match write_draft(&root, &mut draft, &Edit::Remove) {
            Ok(()) => self.toast("Poof. Capture undone."),
            Err(error) => self.worklist_error("Can\u{2019}t undo just yet", &error),
        }
    }

    fn worklist_error(&self, title: &str, error: &anyhow::Error) {
        let toast = adw::Toast::new(&format!("{title} \u{2014} {error}"));
        toast.set_use_markup(false);
        toast.set_button_label(Some("Show worklist"));
        toast.set_action_name(Some("app.open-worklist"));
        self.toasts.add_toast(toast);
    }

    /// A toast that puts the worklist file back to `before`, while it still reads `after`.
    pub(crate) fn offer_worklist_undo(
        &self,
        root: std::path::PathBuf,
        text: &str,
        before: String,
        after: String,
    ) {
        let toast = adw::Toast::new(text);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        toast.connect_button_clicked(move |_| {
            let (root, before, after) = (root.clone(), before.clone(), after.clone());
            gtk::glib::idle_add_local_once(move || {
                let result = WorklistFile::new(&root).and_then(|file| {
                    let old = file.read()?;
                    anyhow::ensure!(
                        old.as_deref() == Some(after.as_bytes()),
                        "the worklist changed since"
                    );
                    file.write(old.as_deref(), before.as_bytes())
                });
                if let Err(error) = result {
                    eprintln!("[xg] worklist undo failed: {error}");
                }
            });
        });
        self.toasts.add_toast(toast);
    }

    fn task_added_toast(self: &Rc<Self>, ws: WorkspaceId, draft: ItemDraft) {
        let text = match draft.title() {
            Some(title) => format!("Got it, task added \u{2014} {title}"),
            None => "Got it, task added".to_string(),
        };
        let toast = adw::Toast::new(&text);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        toast.set_action_name(Some("app.undo-capture"));
        let serial = self.worklist.toast_serial.get() + 1;
        self.worklist.toast_serial.set(serial);
        *self.worklist.undo.borrow_mut() = Some((ws, draft));
        let weak = Rc::downgrade(self);
        toast.connect_dismissed(move |_| {
            if let Some(app) = weak.upgrade()
                && app.worklist.toast_serial.get() == serial
            {
                app.worklist.undo.borrow_mut().take();
            }
        });
        self.toasts.add_toast(toast);
    }
}
