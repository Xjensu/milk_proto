//! Bundle extension TLVs (doc 03 §4.4, doc 10 §3).
//!
//! Canonical rules: strictly ascending type (covers duplicates), exact value
//! lengths for fixed-layout types, unknown critical (0x00–0x7F) rejected,
//! unknown optional (0x80–0xFF) retained opaquely for re-encoding.

use alloc::vec::Vec;

use crate::err::WireError;
use crate::ids::Id16;
use crate::wire::{BufExt, Reader};

/// TLV type 0x01: causal-parent references (see [`Ext::Parents`]).
pub const TLV_PARENTS: u8 = 0x01;
/// TLV type 0x02: group key epoch (see [`Ext::GroupEpoch`]).
pub const TLV_GROUP_EPOCH: u8 = 0x02;
/// TLV type 0x03: blob symbol coordinates (see [`Ext::SymbolInfo`]).
pub const TLV_SYMBOL_INFO: u8 = 0x03;
/// TLV type 0x80: traffic-shape padding (see [`Ext::Padding`]).
pub const TLV_PADDING: u8 = 0x80;
/// Optional-range (doc 18 §5.2.1): a self-contained AuthCert attached to an
/// elevated-priority announcement. Deliberately NOT a typed `Ext` variant — a
/// malformed attach must still decode the bundle (relay-blind per doc 03 §4.4
/// ignore-if-unknown); it degrades to "no cert" only at the announcement layer
/// (`mmp-node`'s `handle_announcement`), never to a bundle-decode rejection.
pub const TLV_AUTHCERT_ATTACH: u8 = 0x81;
/// Optional-range (doc 18 §3): human-readable scope-name hint, display only —
/// never trusted for admission. Same "retain as `Ext::Optional`, no typed
/// variant" treatment as `TLV_AUTHCERT_ATTACH`.
pub const TLV_SCOPE_INFO: u8 = 0x82;

/// Maximum causal parents a single `Parents` TLV may carry.
pub const MAX_PARENTS: usize = 4;

/// One decoded bundle extension TLV. Variants below `0x80` are critical
/// (unknown critical types reject the whole bundle on decode); `0x80` and
/// above are optional and unknown ones round-trip opaquely via
/// [`Ext::Optional`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ext {
    /// 0x01 — causal references, 1–4 BundleIDs (conversation DAG, doc 06 §5).
    Parents(Vec<Id16>),
    /// 0x02 — group key epoch (class 2).
    GroupEpoch(u16),
    /// 0x03 — blob symbol coordinates (class 4).
    SymbolInfo {
        /// Blob root ID this symbol belongs to.
        root: Id16,
        /// Stripe (block) index within the blob.
        block: u16,
        /// Symbol index within the stripe.
        sym: u16,
        /// Reed-Solomon data-symbol count for this stripe.
        k: u8,
        /// Reed-Solomon total (data + parity) symbol count for this stripe.
        n: u8,
    },
    /// 0x80 — traffic-shape padding (value bytes preserved verbatim).
    Padding(Vec<u8>),
    /// 0x81–0xFF — unknown optional, parse-and-ignore but re-encodable.
    Optional {
        /// The unrecognized TLV type byte.
        typ: u8,
        /// The TLV's raw value bytes, kept opaque for re-encoding.
        val: Vec<u8>,
    },
}

impl Ext {
    /// The TLV type byte this extension encodes/decoded as.
    pub fn typ(&self) -> u8 {
        match self {
            Ext::Parents(_) => TLV_PARENTS,
            Ext::GroupEpoch(_) => TLV_GROUP_EPOCH,
            Ext::SymbolInfo { .. } => TLV_SYMBOL_INFO,
            Ext::Padding(_) => TLV_PADDING,
            Ext::Optional { typ, .. } => *typ,
        }
    }

    fn encode_value(&self, out: &mut Vec<u8>) {
        match self {
            Ext::Parents(ids) => {
                for id in ids {
                    out.extend_from_slice(&id.0);
                }
            }
            Ext::GroupEpoch(e) => out.put_u16(*e),
            Ext::SymbolInfo {
                root,
                block,
                sym,
                k,
                n,
            } => {
                out.extend_from_slice(&root.0);
                out.put_u16(*block);
                out.put_u16(*sym);
                out.push(*k);
                out.push(*n);
            }
            Ext::Padding(v) | Ext::Optional { val: v, .. } => out.extend_from_slice(v),
        }
    }
}

/// Encode extensions. Caller keeps `exts` sorted ascending by type
/// ([`Ext::typ`]); encode asserts it so a non-canonical form is unrepresentable.
pub fn encode_exts(exts: &[Ext], out: &mut Vec<u8>) {
    let mut prev: i32 = -1;
    for ext in exts {
        let typ = ext.typ();
        debug_assert!(
            i32::from(typ) > prev,
            "extensions must be strictly ascending"
        );
        prev = i32::from(typ);
        out.push(typ);
        let len_at = out.len();
        out.put_u16(0);
        ext.encode_value(out);
        let vlen = (out.len() - len_at - 2) as u16;
        out[len_at..len_at + 2].copy_from_slice(&vlen.to_le_bytes());
    }
}

/// Decode exactly `bytes` as a canonical TLV run.
pub fn decode_exts(bytes: &[u8]) -> Result<Vec<Ext>, WireError> {
    let mut r = Reader::new(bytes);
    let mut exts = Vec::new();
    let mut prev: i32 = -1;
    while r.remaining() > 0 {
        let typ = r.u8()?;
        if i32::from(typ) <= prev {
            return Err(WireError::TlvOrder);
        }
        prev = i32::from(typ);
        let len = r.u16()? as usize;
        let val = r.take(len)?;
        exts.push(decode_one(typ, val)?);
    }
    r.finish()?;
    Ok(exts)
}

fn decode_one(typ: u8, val: &[u8]) -> Result<Ext, WireError> {
    match typ {
        TLV_PARENTS => {
            if val.is_empty() || !val.len().is_multiple_of(16) || val.len() / 16 > MAX_PARENTS {
                return Err(WireError::TlvLength);
            }
            let ids = val
                .chunks_exact(16)
                .map(|c| {
                    let mut a = [0u8; 16];
                    a.copy_from_slice(c);
                    Id16(a)
                })
                .collect();
            Ok(Ext::Parents(ids))
        }
        TLV_GROUP_EPOCH => {
            if val.len() != 2 {
                return Err(WireError::TlvLength);
            }
            Ok(Ext::GroupEpoch(u16::from_le_bytes([val[0], val[1]])))
        }
        TLV_SYMBOL_INFO => {
            if val.len() != 22 {
                return Err(WireError::TlvLength);
            }
            let mut root = [0u8; 16];
            root.copy_from_slice(&val[..16]);
            Ok(Ext::SymbolInfo {
                root: Id16(root),
                block: u16::from_le_bytes([val[16], val[17]]),
                sym: u16::from_le_bytes([val[18], val[19]]),
                k: val[20],
                n: val[21],
            })
        }
        TLV_PADDING => Ok(Ext::Padding(val.to_vec())),
        t if t < 0x80 => Err(WireError::TlvUnknownCritical),
        t => Ok(Ext::Optional {
            typ: t,
            val: val.to_vec(),
        }),
    }
}
