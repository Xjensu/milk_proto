//! mmp-types — ids, frames, bundles, canonical codec (doc 10).
//!
//! This crate IS the wire spec: hand-written canonical codec, no serde on the
//! wire, ever (doc 12 §4). Exactly one valid byte representation of any value;
//! parsers reject trailing bytes, overlong lengths, duplicate or non-ascending
//! TLV types, non-zero reserved bits (doc 10 §6.1).
//!
//! Layering (doc 12 §1): `#![no_std]` + `alloc`.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

/// Authority certificates, announcement bodies, and revocation lists (doc 18).
pub mod authcert;
/// Beacon payload (doc 10 §5): the fixed-size, unauthenticated pre-connection
/// advertisement.
pub mod beacon;
/// Bundle codec (doc 10 §3): the store-and-forward wire unit and its classes.
pub mod bundle;
/// Claim records (doc 10 §4.4, doc 06 §4): single-writer LWW registers.
pub mod claim;
/// Protocol constants — the single source module (HANDOFF §4, doc 03 §9).
pub mod consts;
/// Global Ed25519 signing-domain separation (doc 10 §3.1, AUDIT F4).
pub mod domain;
/// Wire-format errors (doc 10 §6 canonical-encoding rules).
pub mod err;
/// Session frame envelope and every v1 frame type (doc 10 §2).
pub mod frame;
/// Candidate group membership control and bootstrap codecs.
pub mod group_control;
/// Fixed-size wire primitives (doc 10 §1) and NodeID derivation (doc 03 §3.2).
pub mod ids;
/// FileManifest (doc 16 §2): a signed share of 1..16 files.
pub mod manifest;
/// Per-class bundle payload schemas (doc 10 §4).
pub mod payload;
/// Bundle extension TLVs (doc 03 §4.4, doc 10 §3).
pub mod tlv;
/// Canonical byte reader/writer; all integers little-endian (doc 10 §1).
pub mod wire;

pub use beacon::Beacon;
pub use bundle::{class, Bundle};
pub use claim::{Claim, Friendship, PrekeyBatch, Revocation};
pub use domain::{signed_preimage, SigDomain};
pub use err::WireError;
pub use frame::{Frame, Hello, Range, Summary};
pub use group_control::{
    AddTag, AdminInfo, AdminKey, AnchorAck, BootstrapDistribution, BootstrapInvite, EpochAnchor,
    EpochAnchorSignature, GroupControlBatch, SenderKeyDist, SenderKeyGrant, SignedMembershipOp,
};
pub use ids::{node_id_of, Id16, Key32, NodeId, Sig64};
pub use manifest::{FileEntry, FileManifest};
pub use payload::{AsyncPayload, DirectPayload, GroupPayload, ReceiptPayload, SymbolPayload};
pub use tlv::Ext;

#[cfg(test)]
mod tests;
