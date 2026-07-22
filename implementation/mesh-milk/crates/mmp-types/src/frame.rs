//! Session frame envelope and every v1 frame type (doc 10 §2).
//!
//! `Frame { len: u16 ≤ 4096, typ: u8, payload }`. Unknown type ⇒ the session
//! answers ERROR(unsupported) + CLOSE — no tolerant parsing (doc 10 §2).

use alloc::vec::Vec;

use crate::consts::MAX_FRAME_PAYLOAD;
use crate::err::WireError;
use crate::ids::{Id16, Key32, Sig64};
use crate::wire::{BufExt, Reader};

pub const FT_HS: u8 = 0x01;
pub const FT_ID_PROOF: u8 = 0x02;
pub const FT_CREDIT: u8 = 0x03;
pub const FT_SUMMARY: u8 = 0x10;
pub const FT_RECON_INIT: u8 = 0x11;
pub const FT_RECON_RANGE: u8 = 0x12;
pub const FT_RECON_IDS: u8 = 0x13;
pub const FT_WANT: u8 = 0x14;
pub const FT_BUNDLE: u8 = 0x20;
pub const FT_BUDGET: u8 = 0x21;
pub const FT_BLOB_REQ: u8 = 0x30;
pub const FT_BLOB_HAVE: u8 = 0x31;
pub const FT_FRIEND_OFFER: u8 = 0x40;
pub const FT_FRIEND_ACK: u8 = 0x41;
pub const FT_PROXY_SUMMARY: u8 = 0x42;
pub const FT_ERROR: u8 = 0x7e;
pub const FT_CLOSE: u8 = 0x7f;

/// ERROR codes (doc 10 §2: `code u8 + varbytes detail`).
pub mod errcode {
    pub const UNSUPPORTED: u8 = 0x01;
    pub const PROTOCOL: u8 = 0x02;
    pub const OVERSIZE: u8 = 0x03;
}

/// RBSR range descriptor: `lo id16, hi id16, count u32, fp u64` (doc 10 §2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub lo: Id16,
    pub hi: Id16,
    pub count: u32,
    pub fp: u64,
}

impl Range {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.lo.0);
        out.extend_from_slice(&self.hi.0);
        out.put_u32(self.count);
        out.put_u64(self.fp);
    }
    fn decode(r: &mut Reader<'_>) -> Result<Range, WireError> {
        Ok(Range { lo: r.id16()?, hi: r.id16()?, count: r.u32()?, fp: r.u64()? })
    }
}

/// SUMMARY contents (doc 10 §2.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub receipt_buckets: Vec<SummaryBucket>,
    pub claim_digest: Vec<ClaimDigestEntry>,
    pub group_hints: Vec<Id16>,
    pub want_heads: Vec<WantHeads>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryBucket {
    pub bucket_epoch: u16,
    pub bloom: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimDigestEntry {
    pub node_prefix: u32,
    pub field: u8,
    pub seq: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantHeads {
    pub conv: Id16,
    pub heads: Vec<Id16>,
}

/// FRIEND_OFFER body (handbook §4.5, doc 04 §4.1): `since u64`. No signature
/// inside — the session is mutually authenticated post-ID_PROOF, so offer
/// authenticity comes from the AEAD channel, not a body signature. Direction:
/// the dependent solicits from the patron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriendOfferBody {
    pub since: u64,
}

impl FriendOfferBody {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.put_u64(self.since);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<FriendOfferBody, WireError> {
        let mut r = Reader::new(bytes);
        let since = r.u64()?;
        r.finish()?;
        Ok(FriendOfferBody { since })
    }
}

/// FRIEND_ACK body (handbook §4.5): `countersig sig64` — the patron's
/// signature over [`crate::claim::friendship_signed_object`] under
/// `SigDomain::Friend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriendAckBody {
    pub countersig: Sig64,
}

impl FriendAckBody {
    pub fn encode(&self) -> Vec<u8> {
        self.countersig.0.to_vec()
    }
    pub fn decode(bytes: &[u8]) -> Result<FriendAckBody, WireError> {
        let mut r = Reader::new(bytes);
        let countersig = r.sig64()?;
        r.finish()?;
        Ok(FriendAckBody { countersig })
    }
}

/// PROXY_SUMMARY body (handbook §4.5): `dependent id16, bloom varbytes` — one
/// frame per dependent the patron currently buffers for; advisory
/// spray-suppression hint only (doc 06 §7 MVP, AUDIT F11 Hint/Purge shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxySummaryBody {
    pub dependent: Id16,
    pub bloom: Vec<u8>,
}

impl ProxySummaryBody {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 2 + self.bloom.len());
        out.extend_from_slice(&self.dependent.0);
        out.put_var(&self.bloom);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<ProxySummaryBody, WireError> {
        let mut r = Reader::new(bytes);
        let dependent = r.id16()?;
        let bloom = r.varbytes(MAX_FRAME_PAYLOAD)?.to_vec();
        r.finish()?;
        Ok(ProxySummaryBody { dependent, bloom })
    }
}

/// HELLO, carried inside the Noise handshake payloads (doc 10 §2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub ver: u8,
    pub suites: Vec<u8>,
    pub tier: u8,
    pub flags: u16,
    pub credits: u8,
    pub psm: u16,
}

impl Hello {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(9 + self.suites.len());
        out.push(self.ver);
        out.put_var(&self.suites);
        out.push(self.tier);
        out.put_u16(self.flags);
        out.push(self.credits);
        out.put_u16(self.psm);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<Hello, WireError> {
        let mut r = Reader::new(bytes);
        let hello = Hello {
            ver: r.u8()?,
            suites: r.varbytes(16)?.to_vec(),
            tier: r.u8()?,
            flags: r.u16()?,
            credits: r.u8()?,
            psm: r.u16()?,
        };
        r.finish()?;
        Ok(hello)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Hs(Vec<u8>),
    IdProof { identity: Key32, sig: Sig64 },
    Credit(u8),
    Summary(Summary),
    ReconInit { domain: u8, window: u8, range: Range },
    ReconRange { domain: u8, ranges: Vec<Range> },
    ReconIds { domain: u8, lo: Id16, hi: Id16, ids: Vec<Id16> },
    Want(Vec<Id16>),
    /// Raw bundle bytes; canonical decode happens at ingest, not framing.
    Bundle(Vec<u8>),
    Budget { id: Id16, granted: u8 },
    BlobReq { root: Id16, block: u16, bitmap: Vec<u8> },
    BlobHave { root: Id16, block: u16, bitmap: Vec<u8> },
    /// Friendship frames: wire types parse (peers must not choke), agent is
    /// stubbed for MVP (HANDOFF §0) — payloads kept opaque.
    FriendOffer(Vec<u8>),
    FriendAck(Vec<u8>),
    ProxySummary(Vec<u8>),
    Error { code: u8, detail: Vec<u8> },
    Close,
}

impl Frame {
    pub fn typ(&self) -> u8 {
        match self {
            Frame::Hs(_) => FT_HS,
            Frame::IdProof { .. } => FT_ID_PROOF,
            Frame::Credit(_) => FT_CREDIT,
            Frame::Summary(_) => FT_SUMMARY,
            Frame::ReconInit { .. } => FT_RECON_INIT,
            Frame::ReconRange { .. } => FT_RECON_RANGE,
            Frame::ReconIds { .. } => FT_RECON_IDS,
            Frame::Want(_) => FT_WANT,
            Frame::Bundle(_) => FT_BUNDLE,
            Frame::Budget { .. } => FT_BUDGET,
            Frame::BlobReq { .. } => FT_BLOB_REQ,
            Frame::BlobHave { .. } => FT_BLOB_HAVE,
            Frame::FriendOffer(_) => FT_FRIEND_OFFER,
            Frame::FriendAck(_) => FT_FRIEND_ACK,
            Frame::ProxySummary(_) => FT_PROXY_SUMMARY,
            Frame::Error { .. } => FT_ERROR,
            Frame::Close => FT_CLOSE,
        }
    }

    pub fn encode_payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Frame::Hs(b) | Frame::FriendOffer(b) | Frame::FriendAck(b) | Frame::ProxySummary(b) => {
                out.extend_from_slice(b)
            }
            Frame::IdProof { identity, sig } => {
                out.extend_from_slice(&identity.0);
                out.extend_from_slice(&sig.0);
            }
            Frame::Credit(c) => out.push(*c),
            Frame::Summary(s) => {
                out.push(s.receipt_buckets.len() as u8);
                for b in &s.receipt_buckets {
                    out.put_u16(b.bucket_epoch);
                    out.put_var(&b.bloom);
                }
                out.push(s.claim_digest.len() as u8);
                for c in &s.claim_digest {
                    out.put_u32(c.node_prefix);
                    out.push(c.field);
                    out.put_u32(c.seq);
                }
                out.push(s.group_hints.len() as u8);
                for g in &s.group_hints {
                    out.extend_from_slice(&g.0);
                }
                out.push(s.want_heads.len() as u8);
                for w in &s.want_heads {
                    out.extend_from_slice(&w.conv.0);
                    out.push(w.heads.len() as u8);
                    for h in &w.heads {
                        out.extend_from_slice(&h.0);
                    }
                }
            }
            Frame::ReconInit { domain, window, range } => {
                out.push(*domain);
                out.push(*window);
                range.encode(&mut out);
            }
            Frame::ReconRange { domain, ranges } => {
                out.push(*domain);
                out.push(ranges.len() as u8);
                for rg in ranges {
                    rg.encode(&mut out);
                }
            }
            Frame::ReconIds { domain, lo, hi, ids } => {
                out.push(*domain);
                out.extend_from_slice(&lo.0);
                out.extend_from_slice(&hi.0);
                out.put_u16(ids.len() as u16);
                for id in ids {
                    out.extend_from_slice(&id.0);
                }
            }
            Frame::Want(ids) => {
                out.put_u16(ids.len() as u16);
                for id in ids {
                    out.extend_from_slice(&id.0);
                }
            }
            Frame::Bundle(b) => out.extend_from_slice(b),
            Frame::Budget { id, granted } => {
                out.extend_from_slice(&id.0);
                out.push(*granted);
            }
            Frame::BlobReq { root, block, bitmap } | Frame::BlobHave { root, block, bitmap } => {
                out.extend_from_slice(&root.0);
                out.put_u16(*block);
                out.put_var(bitmap);
            }
            Frame::Error { code, detail } => {
                out.push(*code);
                out.put_var(detail);
            }
            Frame::Close => {}
        }
        debug_assert!(out.len() <= MAX_FRAME_PAYLOAD);
        out
    }

    /// Full envelope: `len u16 ‖ typ u8 ‖ payload`.
    pub fn encode(&self) -> Vec<u8> {
        let payload = self.encode_payload();
        let mut out = Vec::with_capacity(3 + payload.len());
        out.put_u16(payload.len() as u16);
        out.push(self.typ());
        out.extend_from_slice(&payload);
        out
    }

    pub fn decode_payload(typ: u8, payload: &[u8]) -> Result<Frame, WireError> {
        let mut r = Reader::new(payload);
        let frame = match typ {
            FT_HS => Frame::Hs(payload.to_vec()),
            FT_ID_PROOF => {
                let f = Frame::IdProof { identity: r.key32()?, sig: r.sig64()? };
                r.finish()?;
                f
            }
            FT_CREDIT => {
                let f = Frame::Credit(r.u8()?);
                r.finish()?;
                f
            }
            FT_SUMMARY => {
                let mut s = Summary::default();
                for _ in 0..r.u8()? {
                    s.receipt_buckets.push(SummaryBucket {
                        bucket_epoch: r.u16()?,
                        bloom: r.varbytes(MAX_FRAME_PAYLOAD)?.to_vec(),
                    });
                }
                for _ in 0..r.u8()? {
                    s.claim_digest.push(ClaimDigestEntry {
                        node_prefix: r.u32()?,
                        field: r.u8()?,
                        seq: r.u32()?,
                    });
                }
                for _ in 0..r.u8()? {
                    s.group_hints.push(r.id16()?);
                }
                for _ in 0..r.u8()? {
                    let conv = r.id16()?;
                    let mut heads = Vec::new();
                    for _ in 0..r.u8()? {
                        heads.push(r.id16()?);
                    }
                    s.want_heads.push(WantHeads { conv, heads });
                }
                r.finish()?;
                Frame::Summary(s)
            }
            FT_RECON_INIT => {
                let f = Frame::ReconInit {
                    domain: r.u8()?,
                    window: r.u8()?,
                    range: Range::decode(&mut r)?,
                };
                r.finish()?;
                f
            }
            FT_RECON_RANGE => {
                let domain = r.u8()?;
                let n = r.u8()?;
                let mut ranges = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    ranges.push(Range::decode(&mut r)?);
                }
                r.finish()?;
                Frame::ReconRange { domain, ranges }
            }
            FT_RECON_IDS => {
                let domain = r.u8()?;
                let lo = r.id16()?;
                let hi = r.id16()?;
                let n = r.u16()?;
                let mut ids = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    ids.push(r.id16()?);
                }
                r.finish()?;
                Frame::ReconIds { domain, lo, hi, ids }
            }
            FT_WANT => {
                let n = r.u16()?;
                let mut ids = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    ids.push(r.id16()?);
                }
                r.finish()?;
                Frame::Want(ids)
            }
            FT_BUNDLE => Frame::Bundle(payload.to_vec()),
            FT_BUDGET => {
                let f = Frame::Budget { id: r.id16()?, granted: r.u8()? };
                r.finish()?;
                f
            }
            FT_BLOB_REQ | FT_BLOB_HAVE => {
                let root = r.id16()?;
                let block = r.u16()?;
                let bitmap = r.varbytes(MAX_FRAME_PAYLOAD)?.to_vec();
                r.finish()?;
                if typ == FT_BLOB_REQ {
                    Frame::BlobReq { root, block, bitmap }
                } else {
                    Frame::BlobHave { root, block, bitmap }
                }
            }
            FT_FRIEND_OFFER => Frame::FriendOffer(payload.to_vec()),
            FT_FRIEND_ACK => Frame::FriendAck(payload.to_vec()),
            FT_PROXY_SUMMARY => Frame::ProxySummary(payload.to_vec()),
            FT_ERROR => {
                let code = r.u8()?;
                let detail = r.varbytes(256)?.to_vec();
                r.finish()?;
                Frame::Error { code, detail }
            }
            FT_CLOSE => {
                r.finish()?;
                Frame::Close
            }
            _ => return Err(WireError::BadValue("unknown frame type")),
        };
        Ok(frame)
    }
}

/// Envelope cap on the wire: logical payload cap + AEAD tag headroom for the
/// post-handshake channel (frames after HS travel encrypted, doc 10 §2).
pub const MAX_ENVELOPE_PAYLOAD: usize = MAX_FRAME_PAYLOAD + 32;

/// Incremental envelope reader for a byte stream. Returns `Ok(None)` while the
/// frame is incomplete; the caller's reassembly buffer is pre-allocated to the
/// cap and NEVER grown from the wire (doc 10 §6.4, AUDIT F10) — a prefix over
/// the cap is `Oversize` ⇒ ERROR + CLOSE.
/// `(frame type, payload slice, total bytes consumed)`.
pub type EnvelopeParts<'a> = (u8, &'a [u8], usize);

pub fn read_envelope(buf: &[u8]) -> Result<Option<EnvelopeParts<'_>>, WireError> {
    if buf.len() < 3 {
        return Ok(None);
    }
    let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if len > MAX_ENVELOPE_PAYLOAD {
        return Err(WireError::Oversize);
    }
    let total = 3 + len;
    if buf.len() < total {
        return Ok(None);
    }
    Ok(Some((buf[2], &buf[3..total], total)))
}

/// Write a raw envelope around an (already encrypted) payload.
pub fn write_envelope(typ: u8, payload: &[u8]) -> Vec<u8> {
    debug_assert!(payload.len() <= MAX_ENVELOPE_PAYLOAD);
    let mut out = Vec::with_capacity(3 + payload.len());
    out.put_u16(payload.len() as u16);
    out.push(typ);
    out.extend_from_slice(payload);
    out
}
