//! mmp-types — ids, frames, bundles, canonical codec (doc 10).
//!
//! This crate IS the wire spec: hand-written canonical codec, no serde on the
//! wire, ever (doc 12 §4). Exactly one valid byte representation of any value;
//! parsers reject trailing bytes, overlong lengths, duplicate or non-ascending
//! TLV types, non-zero reserved bits (doc 10 §6.1).
//!
//! Layering (doc 12 §1): `#![no_std]` + `alloc`.

#![no_std]

extern crate alloc;

pub mod authcert;
pub mod beacon;
pub mod bundle;
pub mod claim;
pub mod consts;
pub mod domain;
pub mod err;
pub mod frame;
pub mod ids;
pub mod manifest;
pub mod payload;
pub mod tlv;
pub mod wire;

pub use beacon::Beacon;
pub use bundle::{class, Bundle};
pub use claim::{Claim, Friendship, PrekeyBatch, Revocation};
pub use domain::{signed_preimage, SigDomain};
pub use err::WireError;
pub use frame::{Frame, Hello, Range, Summary};
pub use ids::{node_id_of, Id16, Key32, NodeId, Sig64};
pub use manifest::{FileEntry, FileManifest};
pub use payload::{AsyncPayload, DirectPayload, GroupPayload, ReceiptPayload, SymbolPayload};
pub use tlv::Ext;

#[cfg(test)]
mod tests;
