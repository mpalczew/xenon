//! Field encoding for the wire frames.

use std::io;

use crate::{Exit, Size, TermId};

#[derive(Default)]
pub(crate) struct Enc(pub(crate) Vec<u8>);

impl Enc {
    pub(crate) fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub(crate) fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn id(&mut self, v: TermId) {
        self.0.extend_from_slice(&v.0.to_le_bytes());
    }
    pub(crate) fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v);
    }
    pub(crate) fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    pub(crate) fn strs(&mut self, v: &[String]) {
        self.u32(v.len() as u32);
        for s in v {
            self.str(s);
        }
    }
    pub(crate) fn size(&mut self, v: Size) {
        self.u16(v.cols);
        self.u16(v.rows);
    }
    pub(crate) fn exit(&mut self, v: Option<Exit>) {
        match v {
            None => self.u8(0),
            Some(Exit::Code(c)) => {
                self.u8(1);
                self.i32(c);
            }
            Some(Exit::Signal(s)) => {
                self.u8(2);
                self.i32(s);
            }
        }
    }
}

pub(crate) fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}
