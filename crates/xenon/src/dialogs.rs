//! Dialogs and hand-offs: choose or create a workspace folder, and open a
//! clicked terminal link.

use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_core::{FileRoute, TabId, WorkspaceId};

use crate::app::App;
use crate::links::{Opening, decide, opening_for};
use std::sync::Arc;
use xenon_finder::FileIndex;

fn expand_home(text: &str) -> PathBuf {
    match (text.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        (None, Some(home)) if text == "~" => PathBuf::from(home),
        _ => PathBuf::from(text),
    }
}

impl App {
    /// Folder chooser for "Open workspace".
    pub(crate) fn open_workspace_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Open workspace");
        let weak = Rc::downgrade(self);
        dialog.select_folder(Some(&self.win), None::<&gio::Cancellable>, move |result| {
            if let (Ok(folder), Some(app)) = (result, weak.upgrade())
                && let Some(path) = folder.path()
            {
                app.open_workspace(&path);
            }
        });
    }

    /// "New workspace": type a folder path; it is created when missing.
    pub(crate) fn new_workspace_dialog(self: &Rc<Self>) {
        let dialog =
            adw::AlertDialog::new(Some("New workspace"), Some("Folder to create or open."));
        let entry = gtk::Entry::new();
        entry.set_text("~/src/");
        entry.set_position(-1);
        entry.set_activates_default(true);
        dialog.set_extra_child(Some(&entry));
        dialog.add_responses(&[("cancel", "Cancel"), ("create", "Create")]);
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("create"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        let field = entry.clone();
        dialog.connect_response(None, move |_, response| {
            let path = expand_home(field.text().trim());
            if response != "create" || path.as_os_str().is_empty() {
                return;
            }
            let Some(app) = weak.upgrade() else { return };
            match std::fs::create_dir_all(&path) {
                Ok(()) => app.open_workspace(&path),
                Err(error) => app.toast(&format!("Couldn\u{2019}t create that folder: {error}")),
            }
        });
        dialog.present(Some(&self.win));
        entry.grab_focus();
    }

    /// Ctrl+click landed on `text` in a terminal. Xenon shows what it can (files
    /// in an editor tab of the terminal's workspace, folders in Files); the
    /// default app gets the rest.
    pub(crate) fn open_link(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, text: &str) {
        let Some(view) = self.terms.borrow().get(&(ws, tab)).cloned() else {
            return;
        };
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        self.run_opening(ws, decide(text, &view.live_cwd(), &root));
    }

    fn run_opening(self: &Rc<Self>, ws: WorkspaceId, opening: Opening) {
        match opening {
            Opening::Launch(url) => self.launch(&url),
            Opening::Missing(path) => self.toast(&format!("Not found: {}", path.display())),
            Opening::Reveal(dir) => self.reveal_dir(ws, &dir),
            Opening::Editor(path, line) => self.open_file_at(ws, &path, line),
            Opening::Default(path) => self.open_file(&path),
            Opening::Search(reference, line) => self.search_link(ws, reference, line),
        }
    }

    /// A reference that is not on disk where the terminal is: look for files in
    /// the workspace that end with it. One match opens; several go to the file
    /// finder with the reference typed in; none says so.
    fn search_link(self: &Rc<Self>, ws: WorkspaceId, reference: String, line: u32) {
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        let cached = self.file_indexes.borrow().get(&ws).cloned();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let mut index = cached;
            let mut found = Vec::new();
            // A cached index can be stale: rebuild once before giving up.
            for fresh in [false, true] {
                if index.is_none() || fresh {
                    let dir = root.clone();
                    let Ok(built) = gio::spawn_blocking(move || FileIndex::build(&dir)).await
                    else {
                        return;
                    };
                    let built = Arc::new(built);
                    if let Some(app) = weak.upgrade() {
                        app.file_indexes.borrow_mut().insert(ws, built.clone());
                    }
                    index = Some(built);
                }
                let (idx, wanted) = (index.clone().unwrap(), reference.clone());
                let Ok(list) = gio::spawn_blocking(move || idx.files_ending_with(&wanted)).await
                else {
                    return;
                };
                found = list;
                if !found.is_empty() {
                    break;
                }
            }
            let Some(app) = weak.upgrade() else { return };
            match found.as_slice() {
                [] => app.toast(&format!("Not found: {reference}")),
                [one] => app.run_opening(ws, opening_for(root.join(one), Some(line))),
                _ => app.open_finder_with(&reference),
            }
        });
    }

    pub(crate) fn open_file(self: &Rc<Self>, path: &std::path::Path) {
        let route = {
            let model = self.model.borrow();
            model.registry.route_file(path, model.active())
        };
        match route {
            FileRoute::Workspace(id) if self.model.borrow().active() != Some(id) => {
                self.activate_workspace(id);
            }
            FileRoute::NewWorkspace(dir) => self.open_workspace(&dir),
            _ => {}
        }
        let file = gio::File::for_path(path);
        let weak = Rc::downgrade(self);
        gtk::FileLauncher::new(Some(&file)).launch(
            Some(&self.win),
            gio::Cancellable::NONE,
            move |result| {
                if let (Err(error), Some(app)) = (result, weak.upgrade()) {
                    app.toast(&format!("Couldn\u{2019}t open that file: {error}"));
                }
            },
        );
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.toast(&format!("Opened {name} with the default app"));
    }

    pub(crate) fn launch(&self, uri: &str) {
        let context = gtk::prelude::WidgetExt::display(&self.win).app_launch_context();
        if let Err(error) = gio::AppInfo::launch_default_for_uri(uri, Some(&context)) {
            self.toast(&format!("Couldn\u{2019}t open that file: {error}"));
        }
    }
}
