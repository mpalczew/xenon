//! The socket protocol. Private: only `client` and `service` use it.
//!
//! A frame is `u32 length` then the body; the body is a tag byte and fields. Phone
//! calls ride inside as JSON text, because they are rare and shaped by `xenon_remote`.

use std::io::{self, Read, Write};

use xenon_core::{TermId, WorkspaceId};

use crate::codec::{Enc, bad};
use crate::dec::Dec;
use crate::{Size, TermStatus};

pub(crate) const VERSION: u32 = 2;
const MAX_FRAME: usize = 64 << 20;

pub(crate) enum Request {
    Hello {
        version: u32,
        env: Vec<String>,
    },
    List,
    Create {
        workspace: WorkspaceId,
        cwd: String,
        argv: Vec<String>,
        env: Vec<String>,
    },
    Attach {
        id: TermId,
        size: Size,
    },
    Input {
        id: TermId,
        bytes: Vec<u8>,
    },
    Claim {
        id: TermId,
        size: Size,
    },
    Seen {
        id: TermId,
    },
    Close {
        id: TermId,
    },
    Dark {
        dark: bool,
    },
    Phone {
        json: String,
    },
}

pub(crate) struct Listed {
    pub id: TermId,
    pub workspace: WorkspaceId,
    pub pid: i32,
    pub status: TermStatus,
}

pub(crate) enum Message {
    /// Answer to the request with this sequence number.
    Ok(u64),
    Err(u64, String),
    Created(u64, TermId),
    Listed(u64, Vec<Listed>),
    Reply(u64, String),
    Output {
        id: TermId,
        bytes: Vec<u8>,
    },
    Status {
        id: TermId,
        status: TermStatus,
    },
    Size {
        id: TermId,
        size: Size,
    },
    Born {
        id: TermId,
        workspace: WorkspaceId,
    },
    Closed {
        id: TermId,
    },
    PhoneChanged,
}

fn send(out: &mut impl Write, body: Vec<u8>) -> io::Result<()> {
    let mut frame = (body.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(&body);
    out.write_all(&frame)?;
    out.flush()
}

fn recv(input: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(bad("frame too large"));
    }
    let mut body = vec![0; len];
    input.read_exact(&mut body)?;
    Ok(body)
}

pub(crate) fn write_request(out: &mut impl Write, seq: u64, req: &Request) -> io::Result<()> {
    let mut e = Enc::default();
    e.u64(seq);
    match req {
        Request::Hello { version, env } => {
            e.u8(0);
            e.u32(*version);
            e.strs(env);
        }
        Request::List => e.u8(1),
        Request::Create {
            workspace,
            cwd,
            argv,
            env,
        } => {
            e.u8(2);
            e.ws(*workspace);
            e.str(cwd);
            e.strs(argv);
            e.strs(env);
        }
        Request::Attach { id, size } => {
            e.u8(3);
            e.id(*id);
            e.size(*size);
        }
        Request::Input { id, bytes } => {
            e.u8(4);
            e.id(*id);
            e.bytes(bytes);
        }
        Request::Claim { id, size } => {
            e.u8(5);
            e.id(*id);
            e.size(*size);
        }
        Request::Seen { id } => {
            e.u8(6);
            e.id(*id);
        }
        Request::Close { id } => {
            e.u8(7);
            e.id(*id);
        }
        Request::Dark { dark } => {
            e.u8(8);
            e.u8(u8::from(*dark));
        }
        Request::Phone { json } => {
            e.u8(9);
            e.str(json);
        }
    }
    send(out, e.0)
}

pub(crate) fn read_request(input: &mut impl Read) -> io::Result<(u64, Request)> {
    let body = recv(input)?;
    let mut d = Dec(&body);
    let seq = d.u64()?;
    let req = match d.u8()? {
        0 => Request::Hello {
            version: d.u32()?,
            env: d.strs()?,
        },
        1 => Request::List,
        2 => Request::Create {
            workspace: d.ws()?,
            cwd: d.str()?,
            argv: d.strs()?,
            env: d.strs()?,
        },
        3 => Request::Attach {
            id: d.id()?,
            size: d.size()?,
        },
        4 => Request::Input {
            id: d.id()?,
            bytes: d.bytes()?,
        },
        5 => Request::Claim {
            id: d.id()?,
            size: d.size()?,
        },
        6 => Request::Seen { id: d.id()? },
        7 => Request::Close { id: d.id()? },
        8 => Request::Dark { dark: d.u8()? != 0 },
        9 => Request::Phone { json: d.str()? },
        _ => return Err(bad("unknown request")),
    };
    Ok((seq, req))
}

pub(crate) fn write_message(out: &mut impl Write, msg: &Message) -> io::Result<()> {
    let mut e = Enc::default();
    match msg {
        Message::Ok(seq) => {
            e.u8(0);
            e.u64(*seq);
        }
        Message::Err(seq, text) => {
            e.u8(1);
            e.u64(*seq);
            e.str(text);
        }
        Message::Created(seq, id) => {
            e.u8(2);
            e.u64(*seq);
            e.id(*id);
        }
        Message::Listed(seq, items) => {
            e.u8(3);
            e.u64(*seq);
            e.u32(items.len() as u32);
            for item in items {
                e.id(item.id);
                e.ws(item.workspace);
                e.i32(item.pid);
                e.status(&item.status);
            }
        }
        Message::Reply(seq, json) => {
            e.u8(4);
            e.u64(*seq);
            e.str(json);
        }
        Message::Output { id, bytes } => {
            e.u8(5);
            e.id(*id);
            e.bytes(bytes);
        }
        Message::Status { id, status } => {
            e.u8(6);
            e.id(*id);
            e.status(status);
        }
        Message::Size { id, size } => {
            e.u8(7);
            e.id(*id);
            e.size(*size);
        }
        Message::Born { id, workspace } => {
            e.u8(8);
            e.id(*id);
            e.ws(*workspace);
        }
        Message::Closed { id } => {
            e.u8(9);
            e.id(*id);
        }
        Message::PhoneChanged => e.u8(10),
    }
    send(out, e.0)
}

pub(crate) fn read_message(input: &mut impl Read) -> io::Result<Message> {
    let body = recv(input)?;
    let mut d = Dec(&body);
    Ok(match d.u8()? {
        0 => Message::Ok(d.u64()?),
        1 => Message::Err(d.u64()?, d.str()?),
        2 => Message::Created(d.u64()?, d.id()?),
        3 => {
            let seq = d.u64()?;
            let n = d.u32()?;
            let items = (0..n)
                .map(|_| {
                    Ok(Listed {
                        id: d.id()?,
                        workspace: d.ws()?,
                        pid: d.i32()?,
                        status: d.status()?,
                    })
                })
                .collect::<io::Result<_>>()?;
            Message::Listed(seq, items)
        }
        4 => Message::Reply(d.u64()?, d.str()?),
        5 => Message::Output {
            id: d.id()?,
            bytes: d.bytes()?,
        },
        6 => Message::Status {
            id: d.id()?,
            status: d.status()?,
        },
        7 => Message::Size {
            id: d.id()?,
            size: d.size()?,
        },
        8 => Message::Born {
            id: d.id()?,
            workspace: d.ws()?,
        },
        9 => Message::Closed { id: d.id()? },
        10 => Message::PhoneChanged,
        _ => return Err(bad("unknown message")),
    })
}
