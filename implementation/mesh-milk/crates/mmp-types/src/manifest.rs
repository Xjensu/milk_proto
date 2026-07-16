//! FileManifest (doc 16 §2) — a signed share of 1..16 files, carried inside a
//! class 1/2 body as apptype 0x01. Caps are checked BEFORE allocation.

use alloc::vec::Vec;

use crate::consts::{ALBUM_MAX_FILES, MIME_MAX, NAME_MAX};
use crate::err::WireError;
use crate::ids::{Id16, Key32, Sig64};
use crate::wire::{BufExt, Reader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: Vec<u8>,
    pub mime: Vec<u8>,
    pub size: u32,
    pub file_root: Id16,
    pub rs_k: u8,
    pub rs_n: u8,
    pub full_hash: [u8; 32],
    /// Present iff flags bit0 (has_thumbnail).
    pub thumb_root: Option<Id16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileManifest {
    pub suite: u8,
    /// Original sharer's identity key, inline — verifies standalone (CR2bis-1).
    pub sharer: Key32,
    pub entries: Vec<FileEntry>,
    /// `mmp1/manifest` over `suite ‖ sharer ‖ count ‖ entries`.
    pub sig: Sig64,
}

impl FileEntry {
    fn encode(&self, out: &mut Vec<u8>) {
        out.put_var(&self.name);
        out.put_var(&self.mime);
        out.put_u32(self.size);
        out.extend_from_slice(&self.file_root.0);
        out.push(self.rs_k);
        out.push(self.rs_n);
        out.extend_from_slice(&self.full_hash);
        out.push(if self.thumb_root.is_some() { 1 } else { 0 });
        if let Some(t) = &self.thumb_root {
            out.extend_from_slice(&t.0);
        }
    }

    fn decode(r: &mut Reader<'_>) -> Result<FileEntry, WireError> {
        let name = r.varbytes(NAME_MAX)?.to_vec();
        let mime = r.varbytes(MIME_MAX)?.to_vec();
        let size = r.u32()?;
        let file_root = r.id16()?;
        let rs_k = r.u8()?;
        let rs_n = r.u8()?;
        let mut full_hash = [0u8; 32];
        full_hash.copy_from_slice(r.take(32)?);
        let flags = r.u8()?;
        if flags & !1 != 0 {
            return Err(WireError::ReservedBits);
        }
        let thumb_root = if flags & 1 != 0 { Some(r.id16()?) } else { None };
        Ok(FileEntry { name, mime, size, file_root, rs_k, rs_n, full_hash, thumb_root })
    }
}

impl FileManifest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.suite);
        out.extend_from_slice(&self.sharer.0);
        out.push(self.entries.len() as u8);
        for e in &self.entries {
            e.encode(&mut out);
        }
        out.extend_from_slice(&self.sig.0);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<FileManifest, WireError> {
        let mut r = Reader::new(bytes);
        let suite = r.u8()?;
        let sharer = r.key32()?;
        let count = r.u8()? as usize;
        if count == 0 || count > ALBUM_MAX_FILES {
            return Err(WireError::BadValue("manifest count"));
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(FileEntry::decode(&mut r)?);
        }
        let sig = r.sig64()?;
        r.finish()?;
        Ok(FileManifest { suite, sharer, entries, sig })
    }

    /// The `object_bytes` under the `mmp1/manifest` domain tag.
    pub fn signed_object(&self) -> Vec<u8> {
        let mut o = Vec::new();
        o.push(self.suite);
        o.extend_from_slice(&self.sharer.0);
        o.push(self.entries.len() as u8);
        for e in &self.entries {
            e.encode(&mut o);
        }
        o
    }
}
