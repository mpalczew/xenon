use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_core::{FileRoute, TabId, WorkspaceId};

use crate::app::App;
use crate::links::{Opening, decide, opening_for};
use std::sync::Arc;
use xenon_finder::FileIndex;

pub(crate) struct Prompt {
    pub title: &'static str,
    pub detail: Option<&'static str>,
    pub ok: &'static str,
    pub initial: String,
}

pub(crate) async fn ask_name(
    parent: &impl IsA<gtk::Widget>,
    prompt: Prompt,
    validate: impl Fn(&str) -> Result<String, String> + 'static,
) -> Option<String> {
    let dialog = adw::AlertDialog::new(Some(prompt.title), prompt.detail);
    dialog.add_responses(&[("cancel", "Cancel"), ("ok", prompt.ok)]);
    dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    dialog.set_response_enabled("ok", validate(&prompt.initial).is_ok());
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("cancel");
    let entry = gtk::Entry::new();
    entry.set_text(&prompt.initial);
    entry.set_activates_default(true);
    let note = gtk::Label::new(None);
    note.add_css_class("error");
    note.set_xalign(0.0);
    note.set_wrap(true);
    note.set_visible(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&entry);
    column.append(&note);
    dialog.set_extra_child(Some(&column));
    dialog.set_focus(Some(&entry));

    let validate = Rc::new(validate);
    let initial = prompt.initial.clone();
    let (weak_dialog, weak_note, check) = (dialog.downgrade(), note.downgrade(), validate.clone());
    entry.connect_changed(move |entry| {
        let (Some(dialog), Some(note)) = (weak_dialog.upgrade(), weak_note.upgrade()) else {
            return;
        };
        let text = entry.text();
        let quiet = text.trim().is_empty() || text == initial;
        let check = check.clone();
        glib::idle_add_local_once(move || {
            let result = check(&text);
            note.set_visible(!quiet && result.is_err());
            note.set_text(result.as_ref().err().map_or("", String::as_str));
            dialog.set_response_enabled("ok", result.is_ok());
        });
    });
    let (field, stem) = (
        entry.clone(),
        xenon_finder::fileops::stem_chars(&prompt.initial),
    );
    let name_like = !prompt.initial.contains('/');
    glib::idle_add_local_once(move || {
        field.grab_focus();
        if name_like && stem > 0 {
            field.select_region(0, stem as i32);
        } else {
            field.set_position(-1);
        }
    });
    if dialog.choose_future(Some(parent)).await != "ok" {
        return None;
    }
    validate(&entry.text()).ok()
}

fn check_folder(text: &str) -> Result<String, String> {
    let path = xenon_finder::discover::expand_user_path(text)
        .ok_or("Start with ~/ or / to name a folder.")?;
    if path.exists() && !path.is_dir() {
        return Err("That path is a file, not a folder.".into());
    }
    Ok(path.to_string_lossy().into_owned())
}

impl App {
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

    pub(crate) fn new_workspace_dialog(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let win = self.win.clone();
        glib::spawn_future_local(async move {
            let prompt = Prompt {
                title: "New workspace",
                detail: Some("Folder to create or open."),
                ok: "Create",
                initial: "~/src/".into(),
            };
            let Some(text) = ask_name(&win, prompt, check_folder).await else {
                return;
            };
            let Some(app) = weak.upgrade() else { return };
            let path = PathBuf::from(text);
            match std::fs::create_dir_all(&path) {
                Ok(()) => app.open_workspace(&path),
                Err(error) => app.toast(&format!("Couldn\u{2019}t create that folder: {error}")),
            }
        });
    }

    pub(crate) fn open_link(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, text: &str) {
        let Some(view) = self.term_view(ws, tab) else {
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
            Opening::Editor(path, line) => self.open_file_at(ws, &path, line, 1),
            Opening::Default(path) => self.open_file(&path),
            Opening::Search(reference, line) => self.search_link(ws, reference, line),
        }
    }

    fn search_link(self: &Rc<Self>, ws: WorkspaceId, reference: String, line: u32) {
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        let cached = self.file_indexes.borrow().get(&ws).cloned();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let mut index = cached;
            let mut found = Vec::new();
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
