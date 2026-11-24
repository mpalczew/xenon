use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::{FileRoute, SplitAxis, WorkspaceId};
use xenon_store::{IpcRequest, OpenFileSpec};

use crate::app::App;

pub(crate) fn serve(app: &Rc<App>, listener: std::os::unix::net::UnixListener) {
    type Reply = std::sync::mpsc::Sender<Result<Option<String>, String>>;
    let (tx, rx) = async_channel::unbounded::<(IpcRequest, Reply)>();
    xenon_store::serve_forever(listener, move |req| {
        let (reply, answer) = std::sync::mpsc::channel();
        let _ = tx.send_blocking((req, reply));
        answer
            .recv_timeout(std::time::Duration::from_millis(1_500))
            .unwrap_or(Ok(None))
    });
    let weak = Rc::downgrade(app);
    glib::spawn_future_local(async move {
        while let Ok((req, reply)) = rx.recv().await {
            let Some(app) = weak.upgrade() else { break };
            let _ = reply.send(app.handle_request(req));
        }
    });
}

impl App {
    pub(crate) fn handle_request(
        self: &Rc<Self>,
        req: IpcRequest,
    ) -> Result<Option<String>, String> {
        let result = match req {
            IpcRequest::Pair => return self.pair_text().map(Some),
            IpcRequest::Quit { end } => {
                if end {
                    self.end_terminals_now();
                } else {
                    self.leave();
                }
                return Ok(None);
            }
            IpcRequest::Open { paths } => {
                for path in paths {
                    let _ = self.open_path(OpenFileSpec {
                        path,
                        line: None,
                        column: None,
                        workspace: None,
                    });
                }
                Ok(None)
            }
            IpcRequest::OpenFile(spec) => self.open_path(spec).map(|()| None),
            IpcRequest::Activate => Ok(None),
            IpcRequest::CloseWorkspace { workspace } => return self.close_from_phone(workspace),
        };
        self.win.present();
        result
    }

    fn close_from_phone(self: &Rc<Self>, ws: WorkspaceId) -> Result<Option<String>, String> {
        let open = self.model.borrow().registry.workspace(ws).is_some();
        if !open {
            return Ok(None);
        }
        if let Some(file) = self.model.borrow().dirty_editor(ws) {
            return Err(format!("unsaved changes in {file}"));
        }
        self.close_workspace_id(ws);
        self.save_registry();
        Ok(None)
    }

    fn open_path(self: &Rc<Self>, spec: OpenFileSpec) -> Result<(), String> {
        let named = match &spec.workspace {
            Some(key) => Some(
                self.model
                    .borrow()
                    .registry
                    .find_workspace(key)
                    .ok_or_else(|| format!("no open workspace matches \"{key}\""))?,
            ),
            None => None,
        };
        let path: PathBuf = spec.path.canonicalize().unwrap_or(spec.path);
        if path.is_dir() {
            self.open_workspace(&path);
            return Ok(());
        }
        if !path.is_file() {
            return Err(format!("not a file or folder: {}", path.display()));
        }
        let route = match named {
            Some(id) => FileRoute::Workspace(id),
            None => {
                let model = self.model.borrow();
                model.registry.route_file(&path, model.active())
            }
        };
        match route {
            FileRoute::Workspace(id) => self.activate_workspace(id),
            FileRoute::NewWorkspace(root) => self.open_workspace(&root),
            FileRoute::Current => {}
        }
        let active = self.model.borrow().active();
        if let Some(ws) = active {
            self.aim_pane(ws);
            self.open_file_at(
                ws,
                Path::new(&path),
                spec.line.unwrap_or(1),
                spec.column.unwrap_or(1),
            );
        }
        Ok(())
    }

    fn aim_pane(&self, ws: WorkspaceId) {
        let mut model = self.model.borrow_mut();
        let leaves = model
            .spaces
            .get(&ws)
            .map_or(0, |s| s.content.leaf_ids().len());
        if leaves > 1 {
            model.step_pane(ws, 1);
        } else {
            model.split(ws, SplitAxis::Horizontal);
        }
    }
}
