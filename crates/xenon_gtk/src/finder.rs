//! Ctrl+P: the file finder. The index (`xenon_finder::FileIndex`) is built on a
//! worker thread, queries run on a worker thread after a short debounce, and
//! only the latest answer is drawn. Ranking is `xenon_finder`'s; this turns
//! its matches into palette rows and says what Enter does.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk::{gio, glib};
use xenon_core::WorkspaceId;
use xenon_finder::{FileIndex, FileMatch};

use crate::app::App;
use crate::palette::{Handlers, Options, PaletteRow};

/// Wait this long after the last keystroke before scoring.
const DEBOUNCE: Duration = Duration::from_millis(40);
/// Recently opened files kept per workspace for ranking.
const MAX_RECENTS: usize = 50;

/// One open finder.
struct Session {
    ws: WorkspaceId,
    root: PathBuf,
    recents: Vec<PathBuf>,
    index: RefCell<Option<Arc<FileIndex>>>,
    /// Bumped per query; an answer for an older number is dropped.
    generation: Cell<u64>,
    /// The palette opening this session belongs to.
    epoch: Cell<u64>,
    matches: RefCell<Vec<FileMatch>>,
}

impl App {
    /// Record `path` (absolute) as the most recently opened file of `ws`.
    pub(crate) fn touch_recent(&self, ws: WorkspaceId, path: &std::path::Path) {
        let Some(root) = self.model.borrow().rec(ws).map(|r| r.root.clone()) else {
            return;
        };
        let Ok(rel) = path.strip_prefix(&root) else {
            return;
        };
        let mut recents = self.recents.borrow_mut();
        let list = recents.entry(ws).or_default();
        list.retain(|p| p != rel);
        list.insert(0, rel.to_path_buf());
        list.truncate(MAX_RECENTS);
    }

    pub(crate) fn open_finder(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        let root = {
            let model = self.model.borrow();
            match model.rec(ws) {
                Some(rec) if rec.ssh.is_none() => rec.root.clone(),
                _ => {
                    drop(model);
                    self.toast("The finder does not search SSH workspaces");
                    return;
                }
            }
        };
        let session = Rc::new(Session {
            ws,
            root: root.clone(),
            recents: self.recents.borrow().get(&ws).cloned().unwrap_or_default(),
            index: RefCell::new(self.file_indexes.borrow().get(&ws).cloned()),
            generation: Cell::new(0),
            epoch: Cell::new(0),
            matches: RefCell::default(),
        });
        let have_index = session.index.borrow().is_some();
        let (weak_q, weak_a) = (Rc::downgrade(self), Rc::downgrade(self));
        let (sq, sa) = (session.clone(), session.clone());
        self.launcher.open(
            &self.win,
            Handlers {
                placeholder: if have_index {
                    "Search files…"
                } else {
                    "Indexing…"
                }
                .into(),
                hint: "return opens  ·  esc closes".into(),
                options: Options::default(),
                on_query: Box::new(move |query| {
                    if let Some(app) = weak_q.upgrade() {
                        app.finder_query(&sq, query);
                    }
                }),
                on_accept: Box::new(move |row| {
                    if let Some(app) = weak_a.upgrade() {
                        app.finder_accept(&sa, row);
                    }
                }),
                on_complete: None,
            },
        );
        session.epoch.set(self.launcher.epoch());
        // Build (or refresh) the index off the main thread.
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let built = gio::spawn_blocking(move || FileIndex::build(&root)).await;
            let (Some(app), Ok(index)) = (weak.upgrade(), built) else {
                return;
            };
            let index = Arc::new(index);
            app.file_indexes.borrow_mut().insert(ws, index.clone());
            *session.index.borrow_mut() = Some(index);
            if app.launcher.is_open() && app.launcher.epoch() == session.epoch.get() {
                let query = app.launcher.text();
                app.finder_query(&session, &query);
            }
        });
    }

    fn finder_query(self: &Rc<Self>, session: &Rc<Session>, query: &str) {
        let token = session.generation.get().wrapping_add(1);
        session.generation.set(token);
        let Some(index) = session.index.borrow().clone() else {
            self.launcher.set_rows(Vec::new(), "Indexing workspace…");
            return;
        };
        let (weak, session, query) = (Rc::downgrade(self), session.clone(), query.to_string());
        glib::spawn_future_local(async move {
            if !query.is_empty() {
                glib::timeout_future(DEBOUNCE).await;
            }
            if session.generation.get() != token {
                return;
            }
            let (q, recents) = (query.clone(), session.recents.clone());
            let Ok(found) = gio::spawn_blocking(move || index.query(&q, &recents)).await else {
                return;
            };
            let Some(app) = weak.upgrade() else { return };
            if session.generation.get() != token
                || !app.launcher.is_open()
                || app.launcher.epoch() != session.epoch.get()
            {
                return;
            }
            let rows: Vec<PaletteRow> = found
                .iter()
                .map(|m| {
                    let shown = m.path.to_string_lossy();
                    PaletteRow {
                        title: if m.is_dir {
                            format!("{shown}/")
                        } else {
                            shown.into_owned()
                        },
                        hits: m.hits.clone(),
                        enabled: true,
                        ..PaletteRow::default()
                    }
                })
                .collect();
            let empty = if query.is_empty() {
                "Type to search files"
            } else {
                "No matching files"
            };
            *session.matches.borrow_mut() = found;
            app.launcher.set_rows(rows, empty);
        });
    }

    fn finder_accept(self: &Rc<Self>, session: &Rc<Session>, row: usize) {
        let Some(found) = session.matches.borrow().get(row).cloned() else {
            return;
        };
        self.launcher.close();
        let path = session.root.join(&found.path);
        if found.is_dir {
            self.reveal_dir(session.ws, &path);
        } else {
            self.open_editor(session.ws, &path);
        }
    }
}
