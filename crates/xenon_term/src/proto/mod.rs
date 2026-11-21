//! The socket protocol between a holder and its clients, shared by builds that live
//! side by side for weeks. Private: only `hold`, `terminals` and `mirror` use it.
//!
//! A frame is `u32 length | kind u8 | body`. Kind 0 is control: a JSON object tagged
//! `"t"`. Kind 1 is data: raw bytes, input going to the holder and output coming back.
//! Other kinds are skipped.
//!
//! Three rules make growth free. A decoder ignores fields it does not know. A decoder
//! skips control messages it does not know. A message's meaning and existing fields
//! never change: that is a new major, with its decoder alone in `proto/v<N>.rs`.
//! `Hello`, `Welcome` and `Close` are frozen in every major, so any window can always
//! list, mark and close any holder.

pub(crate) mod v1;

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use xenon_core::WorkspaceId;

/// The major this build speaks, and its minor (informational: which additions exist).
pub(crate) const MAJOR: u16 = 1;
pub(crate) const MINOR: u16 = 0;
const MAX_FRAME: usize = 64 << 20;
const HANDSHAKE: Duration = Duration::from_secs(2);
/// A client that cannot write for this long is not worth blocking the window for.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

const CONTROL: u8 = 0;
const DATA: u8 = 1;

/// The messages that never change, in any major.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum Frozen {
    Hello {
        major: u16,
        minor: u16,
    },
    Welcome {
        major: u16,
        minor: u16,
        build: String,
        workspace: WorkspaceId,
        /// The child's pid.
        pid: i32,
        #[serde(default)]
        cwd: String,
    },
    /// Hang up the terminal's session, give it a grace period, then exit.
    Close,
}

pub(crate) enum Frame {
    Control(Vec<u8>),
    Data(Vec<u8>),
}

/// What a control frame decoded to: a frozen message, a body of the connection's major,
/// or something this build does not know (skip it).
pub(crate) enum Control<T> {
    Frozen(Frozen),
    Body(T),
    Unknown,
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Control<T> {
    if let Ok(frozen) = serde_json::from_slice(bytes) {
        Control::Frozen(frozen)
    } else if let Ok(body) = serde_json::from_slice(bytes) {
        Control::Body(body)
    } else {
        Control::Unknown
    }
}

fn send(out: &mut impl Write, kind: u8, body: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(body.len() + 5);
    frame.extend_from_slice(&((body.len() + 1) as u32).to_le_bytes());
    frame.push(kind);
    frame.extend_from_slice(body);
    out.write_all(&frame)?;
    out.flush()
}

pub(crate) fn write_control(out: &mut impl Write, msg: &impl Serialize) -> io::Result<()> {
    send(out, CONTROL, &encode(msg))
}

/// A control body already encoded by `encode`.
pub(crate) fn write_raw_control(out: &mut impl Write, body: &[u8]) -> io::Result<()> {
    send(out, CONTROL, body)
}

pub(crate) fn encode(msg: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(msg).unwrap_or_default()
}

pub(crate) fn write_data(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    send(out, DATA, bytes)
}

/// The next control or data frame. Frames of other kinds are skipped.
pub(crate) fn read_frame(input: &mut impl Read) -> io::Result<Frame> {
    loop {
        let mut len = [0; 4];
        input.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len) as usize;
        if len == 0 || len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad frame length",
            ));
        }
        let mut body = vec![0; len];
        input.read_exact(&mut body)?;
        let kind = body.remove(0);
        match kind {
            CONTROL => return Ok(Frame::Control(body)),
            DATA => return Ok(Frame::Data(body)),
            _ => {}
        }
    }
}

/// What a client learns by connecting to a holder.
pub(crate) struct Connected {
    pub stream: UnixStream,
    pub major: u16,
    pub build: String,
    pub workspace: WorkspaceId,
    pub child_pid: i32,
    pub cwd: String,
    /// The holder process, from `SO_PEERCRED`.
    pub holder_pid: i32,
}

/// Connect, say hello and read the welcome. The read timeout stays on: the caller lifts it.
/// `ConnectionRefused` means a crashed holder.
pub(crate) fn connect(path: &std::path::Path) -> io::Result<Connected> {
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(HANDSHAKE))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let holder_pid = peer_pid(&stream)?;
    write_control(
        &mut stream,
        &Frozen::Hello {
            major: MAJOR,
            minor: MINOR,
        },
    )?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "no welcome");
    let Frame::Control(first) = read_frame(&mut stream)? else {
        return Err(bad());
    };
    let Control::<v1::FromHolder>::Frozen(Frozen::Welcome {
        major,
        build,
        workspace,
        pid,
        cwd,
        ..
    }) = decode(&first)
    else {
        return Err(bad());
    };
    Ok(Connected {
        stream,
        major,
        build,
        workspace,
        child_pid: pid,
        cwd,
        holder_pid,
    })
}

fn peer_pid(stream: &UnixStream) -> io::Result<i32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `cred` and `len` are valid for the call, which fills in the peer's credentials.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &mut len,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(cred.pid)
}
