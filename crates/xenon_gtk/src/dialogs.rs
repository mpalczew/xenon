//! Dialogs and hand-offs: choose or create a workspace folder, and open a
//! clicked terminal link.

use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use xenon_core::{FileRoute, TabId, WorkspaceId};

use crate::app::App;
use crate::links::{Link, classify};

fn expand_home(text: &str) -> PathBuf {
    match (text.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        (None, Some(home)) if text == "~" => PathBuf::from(home),
        _ => PathBuf::from(text),
    }
}

impl App {
    /// An SSH workspace: keep its session, say why nothing is drawn.
    pub(crate) fn show_unsupported(self: &Rc<Self>, id: WorkspaceId) {
        self.ensure_slot(id);
        let slots = self.slots.borrow();
        if let Some(slot) = slots.get(&id)
            && slot.holder.first_child().is_none()
        {
            let page = adw::StatusPage::new();
            page.set_icon_name(Some("network-server-symbolic"));
            page.set_title("SSH workspace");
            page.set_description(Some("Its session is kept as saved."));
            slot.holder.append(&page);
        }
    }

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

    /// Ctrl+click landed on `text` in a terminal. Routing is the same as
    /// `xenon open`; until editor tabs exist the file opens in the default app.
    pub(crate) fn open_link(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, text: &str) {
        let Some(view) = self.terms.borrow().get(&(ws, tab)).cloned() else {
            return;
        };
        match classify(text, &view.live_cwd()) {
            Link::Url(url) => self.launch(&url),
            Link::Path(path) if !path.exists() => {
                self.toast(&format!("Not found: {}", path.display()));
            }
            Link::Path(path) if path.is_dir() => self.open_workspace(&path),
            Link::Path(path) => self.open_file(&path),
        }
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
        self.launch(&gio::File::for_path(path).uri());
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
