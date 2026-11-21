//! The client's connection to one holder, and the thread that reads it. A connection
//! that ends means the terminal is gone.

use std::io::{self};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use xenon_core::{TermId, WorkspaceId};

use crate::proto::v1::{FromHolder, ToHolder};
use crate::proto::{self, Connected, Control, Frame};
use crate::terminals::{Early, Shared};
use crate::{Build, Event, TermInfo, TermStatus, builds, dir};

pub(crate) struct HolderLink {
    pub id: TermId,
    pub workspace: WorkspaceId,
    pub child_pid: i32,
    pub holder_pid: i32,
    pub build: String,
    pub cwd: String,
    /// This build can read the holder's major.
    pub speaks: bool,
    out: Mutex<UnixStream>,
    status: Mutex<TermStatus>,
}

impl HolderLink {
    /// Send a message. A broken socket also ends the reader thread, which reports it.
    pub(crate) fn send(&self, msg: &impl Serialize) {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = proto::write_control(&mut *out, msg);
    }

    pub(crate) fn send_data(&self, bytes: &[u8]) {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = proto::write_data(&mut *out, bytes);
    }

    /// Messages only a holder of a major this build reads understand.
    pub(crate) fn tell(&self, msg: &ToHolder) {
        if self.speaks {
            self.send(msg);
        }
    }

    pub(crate) fn info(&self) -> TermInfo {
        TermInfo {
            id: self.id,
            workspace: self.workspace,
            pid: self.child_pid,
            cwd: self.cwd.clone(),
            status: self
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            build: Build {
                label: self.build.clone(),
                older: builds::older(self.holder_pid),
                speaks: self.speaks,
            },
        }
    }
}

/// Connect to a holder, announce it with `Event::Ready` and start reading it.
pub(crate) fn adopt(shared: &Arc<Shared>, id: TermId) -> io::Result<()> {
    let connected =
        proto::connect(&dir::sock_path(id)).inspect_err(|e| dir::sweep_if_dead(id, e))?;
    let Connected {
        stream,
        major,
        build,
        workspace,
        child_pid,
        cwd,
        holder_pid,
    } = connected;
    let mut input = stream.try_clone()?;
    let speaks = major == proto::MAJOR;
    // The holder sends its status right after the welcome: take it now, so `Ready`
    // already carries it.
    let first = if speaks {
        first_status(&mut input)
    } else {
        None
    };
    input.set_read_timeout(None)?;
    let link = Arc::new(HolderLink {
        id,
        workspace,
        child_pid,
        holder_pid,
        build,
        cwd,
        speaks,
        out: Mutex::new(stream),
        status: Mutex::new(first.unwrap_or_default()),
    });
    shared.insert(Arc::clone(&link), |early| replay_early(early, &link));
    shared.emit(Event::Ready {
        id,
        info: link.info(),
    });
    let reader = Arc::clone(shared);
    thread::Builder::new()
        .name("xenon-holder-link".into())
        .spawn(move || {
            read(&reader, &link, input);
            reader.remove(&link);
        })?;
    Ok(())
}

fn read(shared: &Shared, link: &HolderLink, mut input: UnixStream) {
    let id = link.id;
    while let Ok(frame) = proto::read_frame(&mut input) {
        if !link.speaks {
            continue;
        }
        match frame {
            Frame::Data(bytes) => shared.emit(Event::Output { id, bytes }),
            Frame::Control(bytes) => match proto::decode::<FromHolder>(&bytes) {
                Control::Body(FromHolder::Status(status)) => {
                    *link.status.lock().unwrap_or_else(|e| e.into_inner()) = status.clone();
                    shared.emit(Event::Status { id, status });
                }
                Control::Body(FromHolder::Size(size)) => shared.emit(Event::Size { id, size }),
                // The replay that follows is plain output; the view paints it fresh.
                Control::Body(FromHolder::Screen) | Control::Frozen(_) | Control::Unknown => {}
            },
        }
    }
}

fn first_status(input: &mut UnixStream) -> Option<TermStatus> {
    let Ok(Frame::Control(bytes)) = proto::read_frame(input) else {
        return None;
    };
    match proto::decode::<FromHolder>(&bytes) {
        Control::Body(FromHolder::Status(status)) => Some(status),
        _ => None,
    }
}

/// Apply what the window did before the holder was there to hear it.
fn replay_early(early: Early, link: &HolderLink) {
    if !link.speaks {
        return;
    }
    if let Some(size) = early.size {
        link.tell(&ToHolder::Claim { size, label: None });
    }
    if early.attach {
        link.tell(&ToHolder::View { answers: true });
    }
    if !early.input.is_empty() {
        link.send_data(&early.input);
    }
}
