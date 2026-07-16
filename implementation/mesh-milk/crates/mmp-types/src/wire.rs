//! Canonical byte reader/writer. All integers little-endian (doc 10 §1).
//!
//! The reader is strict by construction: every structure decoder must call
//! [`Reader::finish`] (or be embedded in one that does), so trailing bytes are
//! always a rejection, never silently ignored.

use alloc::vec::Vec;

use crate::err::WireError;
use crate::ids::{Id16, Key32, Sig64};

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.remaining() < n {
            return Err(WireError::Truncated);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, WireError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, WireError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u40(&mut self) -> Result<u64, WireError> {
        let b = self.take(5)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], 0, 0, 0]))
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    pub fn id16(&mut self) -> Result<Id16, WireError> {
        let b = self.take(16)?;
        let mut a = [0u8; 16];
        a.copy_from_slice(b);
        Ok(Id16(a))
    }

    pub fn key32(&mut self) -> Result<Key32, WireError> {
        let b = self.take(32)?;
        let mut a = [0u8; 32];
        a.copy_from_slice(b);
        Ok(Key32(a))
    }

    pub fn sig64(&mut self) -> Result<Sig64, WireError> {
        let b = self.take(64)?;
        let mut a = [0u8; 64];
        a.copy_from_slice(b);
        Ok(Sig64(a))
    }

    /// `u16 len ‖ bytes` with a caller-supplied cap (checked before any copy).
    pub fn varbytes(&mut self, max: usize) -> Result<&'a [u8], WireError> {
        let len = self.u16()? as usize;
        if len > max {
            return Err(WireError::Oversize);
        }
        self.take(len)
    }

    /// Reject trailing bytes (doc 10 §6.1).
    pub fn finish(self) -> Result<(), WireError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(WireError::Trailing)
        }
    }
}

/// Little-endian append helpers for `Vec<u8>`.
pub trait BufExt {
    fn put_u16(&mut self, v: u16);
    fn put_u32(&mut self, v: u32);
    fn put_u40(&mut self, v: u64);
    fn put_u64(&mut self, v: u64);
    /// `u16 len ‖ bytes`. Caller guarantees `bytes.len() ≤ u16::MAX` via type caps.
    fn put_var(&mut self, bytes: &[u8]);
}

impl BufExt for Vec<u8> {
    fn put_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u40(&mut self, v: u64) {
        debug_assert!(v <= crate::ids::U40_MAX);
        self.extend_from_slice(&v.to_le_bytes()[..5]);
    }
    fn put_u64(&mut self, v: u64) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_var(&mut self, bytes: &[u8]) {
        debug_assert!(bytes.len() <= u16::MAX as usize);
        self.put_u16(bytes.len() as u16);
        self.extend_from_slice(bytes);
    }
}
