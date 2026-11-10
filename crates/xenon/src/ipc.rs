use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::{FileRoute, SplitAxis, WorkspaceId};
use xenon_store::{IpcRequest, OpenFileSpec, OpenPane};

use crate::app::App;

pub(crate) fn serve(app: &Rc<App>, listener: std::os::unix::net::UnixListener) {
    type Reply = std::sync::mpsc::Sender<Result<(), String>>;
    let (tx, rx) = async_channel::unbounded::<(IpcRequest, Reply)>();
    xenon_store::serve_forever(listener, move |req| {
        let (reply, answer) = std::sync::mpsc::channel();
        let _ = tx.send_blocking((req, reply));
        answer
            .recv_timeout(std::time::Duration::from_millis(1_500))
            .unwrap_or(Ok(()))
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
    pub(crate) fn handle_request(self: &Rc<Self>, req: IpcRequest) -> Result<(), String> {
        let result = match req {
            IpcRequest::Open { paths } => {
                for path in paths {
                    let _ = self.open_path(OpenFileSpec {
                        path,
                        line: None,
                        column: None,
                        end_line: None,
                        pane: Default::default(),
                        workspace: None,
                    });
                }
                Ok(())
            }
            IpcRequest::OpenFile(spec) => self.open_path(spec),
            IpcRequest::Activate => Ok(()),
        };
        self.win.present();
        result
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
            self.aim_pane(ws, spec.pane);
            self.open_file_at(
                ws,
                Path::new(&path),
                spec.line.unwrap_or(1),
                spec.column.unwrap_or(1),
            );
        }
        Ok(())
    }

    fn aim_pane(&self, ws: WorkspaceId, pane: OpenPane) {
        let mut model = self.model.borrow_mut();
        let leaves = model
            .spaces
            .get(&ws)
            .map_or(0, |s| s.content.leaf_ids().len());
        match pane {
            OpenPane::Focused => {}
            OpenPane::Sibling if leaves > 1 => {
                model.step_pane(ws, 1);
            }
            OpenPane::Sibling | OpenPane::SplitRight => {
                model.split(ws, SplitAxis::Horizontal);
            }
        }
    }
}
