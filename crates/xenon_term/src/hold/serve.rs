//! One client connection to the holder.

use std::io;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use super::link::Link;
use super::term::Holder;
use crate::proto::v1::ToHolder;
use crate::proto::{self, Control, Frame, Frozen};
use crate::{Claim, Size};

pub(crate) fn serve(holder: &Arc<Holder>, conn: u64, stream: UnixStream) -> io::Result<()> {
    let mut input = stream.try_clone()?;
    // The first frame must be the client's hello. Its major does not matter here: this
    // holder speaks its own, and the client decides whether it can read that.
    match proto::read_frame(&mut input)? {
        Frame::Control(bytes) => {
            if !matches!(
                proto::decode::<ToHolder>(&bytes),
                Control::Frozen(Frozen::Hello { .. })
            ) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "expected hello"));
            }
        }
        Frame::Data(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "expected hello")),
    }
    let link = Link::start(conn, stream);
    link.send(&Frozen::Welcome {
        major: proto::MAJOR,
        minor: proto::MINOR,
        build: crate::BUILD.into(),
        workspace: holder.workspace,
        pid: holder.pid,
        cwd: holder.cwd.clone(),
    });
    holder.join(link);
    let result = pump(holder, conn, &mut input);
    holder.leave(conn);
    result
}

fn pump(holder: &Holder, conn: u64, input: &mut UnixStream) -> io::Result<()> {
    loop {
        match proto::read_frame(input)? {
            Frame::Data(bytes) => holder.input(conn, &bytes),
            Frame::Control(bytes) => match proto::decode::<ToHolder>(&bytes) {
                Control::Frozen(Frozen::Close) => {
                    if holder.close() {
                        super::exit_now();
                    }
                }
                Control::Body(msg) => body(holder, conn, msg),
                // Hello again, a welcome, or something newer than this build: skipped.
                Control::Frozen(_) | Control::Unknown => {}
            },
        }
    }
}

fn body(holder: &Holder, conn: u64, msg: ToHolder) {
    match msg {
        ToHolder::View { answers } => holder.view(conn, answers),
        ToHolder::Unview => holder.unview(conn),
        ToHolder::Claim { size, label } => holder.claim(conn, claim(size, label)),
        ToHolder::Seen => holder.seen(),
    }
}

fn claim(size: Size, label: Option<String>) -> Claim {
    match label {
        None => Claim::Desk(size),
        Some(label) => Claim::Phone { size, label },
    }
}
