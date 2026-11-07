//! `xenon open` and path handoff. `xenon_store` runs the slot's `ipc.sock` on a
//! thread; each request comes back to the main loop through a channel.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::FileRoute;
use xenon_store::{IpcRequest, OpenFileSpec};

use crate::app::App;

/// Serve `listener` for the life of the app.
pub(crate) fn serve(app: &Rc<App>, listener: std::os::unix::net::UnixListener) {
    let (tx, rx) = async_channel::unbounded::<IpcRequest>();
    xenon_store::serve_forever(listener, move |req| {
        let _ = tx.send_blocking(req);
    });
    let weak = Rc::downgrade(app);
    glib::spawn_future_local(async move {
        while let Ok(req) = rx.recv().await {
            let Some(app) = weak.upgrade() else { break };
            app.handle_request(req);
        }
    });
}

impl App {
    /// Apply one request, then bring the window up.
    pub(crate) fn handle_request(self: &Rc<Self>, req: IpcRequest) {
        match req {
            IpcRequest::Open { paths } => {
                for path in paths {
                    self.open_path(OpenFileSpec {
                        path,
                        line: None,
                        column: None,
                        end_line: None,
                        pane: Default::default(),
                    });
                }
            }
            IpcRequest::OpenFile(spec) => self.open_path(spec),
            IpcRequest::Activate => {}
        }
        self.win.present();
    }

    /// A folder becomes a workspace. A file opens in the workspace that holds
    /// it (`Registry::route_file`), at its line, in the focused pane.
    fn open_path(self: &Rc<Self>, spec: OpenFileSpec) {
        let path: PathBuf = spec.path.canonicalize().unwrap_or(spec.path);
        if path.is_dir() {
            self.open_workspace(&path);
            return;
        }
        if !path.is_file() {
            eprintln!("[xg] open: not a file or folder: {}", path.display());
            return;
        }
        let route = {
            let model = self.model.borrow();
            model.registry.route_file(&path, model.active())
        };
        match route {
            FileRoute::Workspace(id) => self.activate_workspace(id),
            FileRoute::NewWorkspace(root) => self.open_workspace(&root),
            FileRoute::Current => {}
        }
        let active = self.model.borrow().active();
        if let Some(ws) = active {
            self.open_file_at(ws, Path::new(&path), spec.line.unwrap_or(1));
        }
    }
}
