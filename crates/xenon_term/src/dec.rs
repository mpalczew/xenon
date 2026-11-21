//! Field decoding for the wire frames.

use std::io;

use crate::codec::bad;
use crate::{Exit, Size, TermId};

pub(crate) struct Dec<'a>(pub(crate) &'a [u8]);

impl Dec<'_> {
    pub(crate) fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.0.len() < n {
            return Err(bad("short frame"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    pub(crate) fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
    pub(crate) fn u8(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    pub(crate) fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(crate) fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(crate) fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    pub(crate) fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(crate) fn id(&mut self) -> io::Result<TermId> {
        Ok(TermId(u128::from_le_bytes(self.array()?)))
    }
    pub(crate) fn bytes(&mut self) -> io::Result<Vec<u8>> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    pub(crate) fn str(&mut self) -> io::Result<String> {
        String::from_utf8(self.bytes()?).map_err(|_| bad("not utf-8"))
    }
    pub(crate) fn strs(&mut self) -> io::Result<Vec<String>> {
        let n = self.u32()?;
        (0..n).map(|_| self.str()).collect()
    }
    pub(crate) fn size(&mut self) -> io::Result<Size> {
        Size::new(self.u16()?, self.u16()?).ok_or_else(|| bad("zero size"))
    }
    pub(crate) fn exit(&mut self) -> io::Result<Option<Exit>> {
        Ok(match self.u8()? {
            0 => None,
            1 => Some(Exit::Code(self.i32()?)),
            _ => Some(Exit::Signal(self.i32()?)),
        })
    }
}
