//! mmp-core — bundle core, session FSM, sync engine (docs 03/06/11).
//!
//! Sans-IO throughout: no sockets, no radios, no timers of its own. Time is an
//! injected monotonic `now_ms`; wire bytes are pushed/pulled by the host. This
//! is what makes one core run under tokio, in an Android foreground service,
//! and inside a deterministic simulator.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod authority;
pub mod bloom;
pub mod claims;
pub mod dag;
pub mod ingest;
pub mod orset;
pub mod rbsr;
pub mod receiptfilter;
pub mod seen;
#[cfg(feature = "std")]
pub mod session;
pub mod store;

pub use authority::{effective_class5_priority, AuthorityStore, CertReject, RootTrust};
pub use claims::{ClaimMerge, ClaimStore};
pub use dag::ConversationDag;
pub use ingest::{ingest, Disposition, DropReason};
pub use orset::{MembershipOp, OrSet};
pub use rbsr::{fingerprint, IdSource, Reconciler};
pub use receiptfilter::{ReceiptFilter, SuppressSignal};
pub use seen::SeenIndex;
#[cfg(feature = "std")]
pub use session::{Phase, SessionEngine, SessionEvent, SessionHost};
pub use store::{BundleMeta, BundleStore, MemStore, SlotState};

#[cfg(test)]
mod tests;
