//! The socket protocol. Private: only `client` and `service` use it.
//!
//! A frame is `u32 length` then the body; the body is a tag byte and fields. The spike
//! uses a hand-written codec to stay dependency-free; a real build can swap it.

use std::io::{self, Read, Write};

use crate::codec::{Enc, bad};
use crate::dec::Dec;
use crate::{Exit, Size, TermId};

pub const VERSION: u32 = 1;
const MAX_FRAME: usize = 64 << 20;

pub enum Request {
    Hello {
        version: u32,
        env: Vec<String>,
    },
    List,
    Create {
        cwd: String,
        argv: Vec<String>,
        env: Vec<String>,
        size: Size,
    },
    Attach {
        id: TermId,
        size: Size,
    },
    Input {
        id: TermId,
        bytes: Vec<u8>,
    },
    Resize {
        id: TermId,
        size: Size,
    },
    Close {
        id: TermId,
    },
}

pub struct Listed {
    pub id: TermId,
    pub pid: i32,
    pub exit: Option<Exit>,
}

pub enum Message {
    /// Answer to the request with this sequence number.
    Ok(u64),
    Err(u64, String),
    Created(u64, TermId),
    Listed(u64, Vec<Listed>),
    Output {
        id: TermId,
        bytes: Vec<u8>,
    },
    Exited {
        id: TermId,
        exit: Exit,
    },
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

pub fn write_request(out: &mut impl Write, seq: u64, req: &Request) -> io::Result<()> {
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
            cwd,
            argv,
            env,
            size,
        } => {
            e.u8(2);
            e.str(cwd);
            e.strs(argv);
            e.strs(env);
            e.size(*size);
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
        Request::Resize { id, size } => {
            e.u8(5);
            e.id(*id);
            e.size(*size);
        }
        Request::Close { id } => {
            e.u8(6);
            e.id(*id);
        }
    }
    send(out, e.0)
}

pub fn read_request(input: &mut impl Read) -> io::Result<(u64, Request)> {
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
            cwd: d.str()?,
            argv: d.strs()?,
            env: d.strs()?,
            size: d.size()?,
        },
        3 => Request::Attach {
            id: d.id()?,
            size: d.size()?,
        },
        4 => Request::Input {
            id: d.id()?,
            bytes: d.bytes()?,
        },
        5 => Request::Resize {
            id: d.id()?,
            size: d.size()?,
        },
        6 => Request::Close { id: d.id()? },
        _ => return Err(bad("unknown request")),
    };
    Ok((seq, req))
}

pub fn write_message(out: &mut impl Write, msg: &Message) -> io::Result<()> {
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
                e.i32(item.pid);
                e.exit(item.exit);
            }
        }
        Message::Output { id, bytes } => {
            e.u8(4);
            e.id(*id);
            e.bytes(bytes);
        }
        Message::Exited { id, exit } => {
            e.u8(5);
            e.id(*id);
            e.exit(Some(*exit));
        }
    }
    send(out, e.0)
}

pub fn read_message(input: &mut impl Read) -> io::Result<Message> {
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
                        pid: d.i32()?,
                        exit: d.exit()?,
                    })
                })
                .collect::<io::Result<_>>()?;
            Message::Listed(seq, items)
        }
        4 => Message::Output {
            id: d.id()?,
            bytes: d.bytes()?,
        },
        5 => Message::Exited {
            id: d.id()?,
            exit: d.exit()?.ok_or_else(|| bad("exit missing"))?,
        },
        _ => return Err(bad("unknown message")),
    })
}
