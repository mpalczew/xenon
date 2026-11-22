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
    let peer = greet(&mut input)?;
    let link = Link::start(conn, stream);
    link.send(&Frozen::Welcome {
        major: proto::MAJOR,
        minor: proto::MINOR,
        build: crate::BUILD.into(),
        workspace: holder.workspace,
        pid: holder.pid,
        cwd: holder.cwd.clone(),
    });
    if matches!(peer, Peer::Speaks) {
        holder.join(link);
    }
    let result = pump(holder, conn, &mut input, peer);
    if matches!(peer, Peer::Speaks) {
        holder.leave(conn);
    }
    result
}

/// `FrozenOnly`: a client on another major. It gets `Welcome` and may send `Close`, and
/// is never joined to the output.
#[derive(Clone, Copy)]
enum Peer {
    Speaks,
    FrozenOnly,
}

fn greet(input: &mut UnixStream) -> io::Result<Peer> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "expected hello");
    let Frame::Control(bytes) = proto::read_frame(input)? else {
        return Err(bad());
    };
    match proto::decode::<ToHolder>(&bytes) {
        Control::Frozen(Frozen::Hello { major, .. }) if major == proto::MAJOR => Ok(Peer::Speaks),
        Control::Frozen(Frozen::Hello { .. }) => Ok(Peer::FrozenOnly),
        _ => Err(bad()),
    }
}

fn pump(holder: &Holder, conn: u64, input: &mut UnixStream, peer: Peer) -> io::Result<()> {
    loop {
        match proto::read_frame(input)? {
            Frame::Data(_) if matches!(peer, Peer::FrozenOnly) => {}
            Frame::Data(bytes) => holder.input(conn, &bytes),
            Frame::Control(bytes) => match proto::decode::<ToHolder>(&bytes) {
                Control::Frozen(Frozen::Close) => {
                    if holder.close() {
                        super::exit_now();
                    }
                }
                Control::Body(_) if matches!(peer, Peer::FrozenOnly) => {}
                Control::Body(msg) => body(holder, conn, msg),
                Control::Frozen(_) | Control::Unknown => {}
            },
        }
    }
}

fn body(holder: &Holder, conn: u64, msg: ToHolder) {
    match msg {
        ToHolder::View { answers } => holder.view(conn, answers),
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
