use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use xenon_core::WorkspaceId;

use crate::app::App;
use crate::commands::Cmd;

mod capture;
mod fields;
mod list;
mod list_ops;

pub(crate) use list::WorklistList;

use capture::Capture;
use xenon_worklist::{Effect, Item, ItemKey, Op, Place, Undo, apply, ensure, read, revert};

#[derive(Default)]
pub(crate) struct Worklist {
    capture: RefCell<Option<Rc<Capture>>>,
    last_section: RefCell<HashMap<WorkspaceId, Place>>,
    undo: RefCell<Option<(WorkspaceId, ItemKey)>>,
    toast_serial: Cell<u64>,
}

impl App {
    fn workspace_for_worklist(&self) -> Option<(WorkspaceId, std::path::PathBuf, String)> {
        let model = self.model.borrow();
        let ws = model.active()?;
        let rec = model.rec(ws)?;
        Some((ws, rec.root.clone(), rec.name.clone()))
    }

    pub(crate) fn open_worklist(self: &Rc<Self>) {
        let Some((ws, root, _)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let capture = self.worklist.capture.borrow().clone();
        if let Some(capture) = capture {
            capture.dialog.close();
        }
        match ensure(&root) {
            Ok(path) => self.open_editor(ws, &path),
            Err(error) => {
                self.toast_failed("Couldn\u{2019}t open the worklist", &error.to_string());
            }
        }
    }

    pub(crate) fn capture_worklist(self: &Rc<Self>) {
        self.open_capture(None, None);
    }

    fn capture_worklist_into(self: &Rc<Self>, section: Place) {
        self.open_capture(Some(section), None);
    }

    fn edit_worklist_item(self: &Rc<Self>, item: Item) {
        self.open_capture(None, Some(item));
    }

    fn open_capture(self: &Rc<Self>, section: Option<Place>, edit: Option<Item>) {
        let open = self.worklist.capture.borrow().clone();
        if let Some(open) = open {
            open.dialog.close();
            return;
        }
        let Some((ws, root, name)) = self.workspace_for_worklist() else {
            self.no_workspace_toast();
            return;
        };
        let (targets, selected) = if edit.is_some() {
            (Vec::new(), 0)
        } else {
            let targets: Vec<(Place, String)> = read(&root)
                .unwrap_or_default()
                .groups
                .into_iter()
                .map(|group| (group.place, group.label))
                .collect();
            let preferred =
                section.or_else(|| self.worklist.last_section.borrow().get(&ws).cloned());
            let selected = preferred
                .and_then(|want| targets.iter().position(|(place, _)| *place == want))
                .unwrap_or(0);
            (targets, selected)
        };
        let capture = Capture::build(self, (ws, root), &name, (targets, selected), edit);
        *self.worklist.capture.borrow_mut() = Some(capture.clone());
        capture.dialog.present(Some(&self.win));
        capture.title.grab_focus();
    }

    pub(crate) fn undo_capture(self: &Rc<Self>) {
        let Some((ws, item)) = self.worklist.undo.borrow_mut().take() else {
            return;
        };
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        match apply(&root, Op::Remove { item }) {
            Ok(applied) if matches!(applied.effect, Effect::Saved { .. }) => {
                self.toast("Poof. Capture undone.");
            }
            Ok(_) => {
                let error = anyhow::anyhow!("that item changed since");
                self.worklist_error("Can\u{2019}t undo just yet", &error);
            }
            Err(error) => self.worklist_error("Can\u{2019}t undo just yet", &error),
        }
    }

    fn worklist_error(&self, title: &str, error: &anyhow::Error) {
        let toast = adw::Toast::new(&format!("{title} \u{2014} {error}"));
        toast.set_use_markup(false);
        toast.set_button_label(Some("Show worklist"));
        toast.set_action_name(Some(Cmd::OpenWorklist.action()));
        self.toasts.add_toast(toast);
    }

    fn offer_worklist_undo(&self, root: std::path::PathBuf, text: &str, undo: Undo) {
        let toast = adw::Toast::new(text);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        let undo = Rc::new(undo);
        toast.connect_button_clicked(move |_| {
            let (root, undo) = (root.clone(), Rc::clone(&undo));
            gtk::glib::idle_add_local_once(move || {
                if let Err(error) = revert(&root, &undo) {
                    eprintln!("[xg] worklist undo failed: {error}");
                }
            });
        });
        self.toasts.add_toast(toast);
    }

    fn task_added_toast(self: &Rc<Self>, ws: WorkspaceId, item: ItemKey, title: &str) {
        let text = match title {
            "" => "Got it, task added".to_string(),
            title => format!("Got it, task added \u{2014} {title}"),
        };
        let toast = adw::Toast::new(&text);
        toast.set_use_markup(false);
        toast.set_button_label(Some("Undo"));
        toast.set_action_name(Some(Cmd::UndoCapture.action()));
        let serial = self.worklist.toast_serial.get() + 1;
        self.worklist.toast_serial.set(serial);
        *self.worklist.undo.borrow_mut() = Some((ws, item));
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
