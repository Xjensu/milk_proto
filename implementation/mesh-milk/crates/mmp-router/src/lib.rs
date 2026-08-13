//! mmp-router — bounded replication (doc 05): Binary Spray-and-Wait + Focus,
//! locally-observed utility (NO transitivity — the route-poisoning surface is
//! structurally removed), per-origin token buckets, eviction and transfer
//! ordering. No route computation exists; only per-contact decisions.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod quota;
pub mod utility;

use alloc::vec::Vec;

use mmp_core::receiptfilter::SuppressSignal;
use mmp_core::store::BundleMeta;
use mmp_types::consts::{L_MAX, OFFER_MIN_LIFETIME_MS, THETA_FOCUS, THETA_SPRAY};
use mmp_types::ids::{Id16, NodeId};

pub use quota::OriginBuckets;
pub use utility::UtilityState;

/// Origin copy budget: `L = clamp(4, 2·⌈log₂(N̂+1)⌉, 32)` (doc 05 §2.2).
/// Integer math (no_std-safe): ⌈log₂(x)⌉ = 32 − leading_zeros(x−1) for x > 1.
pub fn origin_budget(n_hat: f32) -> u8 {
    let x = if n_hat <= 0.0 {
        1u32
    } else {
        (n_hat as u32).saturating_add(1)
    };
    let ceil_log2 = if x <= 1 {
        0
    } else {
        32 - (x - 1).leading_zeros()
    };
    (2 * ceil_log2 as u8).clamp(4, L_MAX)
}

/// Binary split on spray (doc 05 §2.3): give ⌊b/2⌋, keep the rest.
pub fn spray_split(budget: u8) -> (u8, u8) {
    let give = budget / 2;
    (give, budget - give)
}

/// Per-contact hand-off decision for one carried bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    /// Peer is the destination: transfer regardless of budget (doc 05 §2.5).
    Deliver,
    /// Spray with this granted budget; we retain the remainder.
    Spray {
        /// Copies handed to the peer (from `spray_split`).
        grant: u8,
        /// Copies we keep carrying (from `spray_split`).
        keep: u8,
    },
    /// Budget 1 and the peer is a strictly better carrier: give our only copy
    /// and stop carrying (doc 05 §2.4).
    Focus,
    /// Keep carrying.
    Keep,
}

/// Decide what to do with `meta` when meeting `peer` (doc 05 §2, §5).
#[allow(clippy::too_many_arguments)]
pub fn handoff_decision(
    meta: &BundleMeta,
    peer: &NodeId,
    u_self: f32,
    u_peer: f32,
    peer_is_dest: bool,
    peer_is_origin: bool,
    receipt: SuppressSignal,
    now_ms: u64,
) -> Handoff {
    if peer_is_dest {
        return Handoff::Deliver;
    }
    // A focused copy has already spent its only local grant.  Keep the
    // record for direct delivery, but never mint another relay grant on a
    // later contact (including after a crash-recovered hand-off intent).
    if meta.budget == 0 {
        return Handoff::Keep;
    }
    // Never offer back to the origin; purged bundles are gone before this.
    if peer_is_origin || *peer == meta.origin {
        return Handoff::Keep;
    }
    // Exact receipt = the bundle is purged upstream; Bloom hit = deprioritize
    // this offer only (AUDIT F11).
    if receipt != SuppressSignal::None {
        return Handoff::Keep;
    }
    // Items about to die are not worth a transfer (doc 06 §2).
    if meta.remaining_ms(now_ms) < OFFER_MIN_LIFETIME_MS {
        return Handoff::Keep;
    }
    if meta.budget > 1 {
        // Mostly-blind spray with a mild bias against strictly worse carriers.
        if u_peer >= u_self - THETA_SPRAY {
            let (grant, keep) = spray_split(meta.budget);
            return Handoff::Spray { grant, keep };
        }
        return Handoff::Keep;
    }
    if u_peer > u_self + THETA_FOCUS {
        return Handoff::Focus;
    }
    Handoff::Keep
}

/// Transfer ordering within a contact (doc 05 §6): value density under an
/// unpredictable deadline. Returns indices of `metas` in send order.
///
/// Keys on the DECLARED `priority`, never `effective_priority` — doc 18
/// §5.2.2 is explicit that the admission/rendering clamp is a receiver-local
/// scheduling interpretation, not a wire-phase ordering rule: two honest
/// peers can compute different `effective_priority` for the same bundle
/// (their AuthCert views and bucket state differ), so wire-visible ordering
/// must stay keyed on the signed byte both peers agree on.
pub fn transfer_order(metas: &[BundleMeta], peer: &NodeId, now_ms: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..metas.len()).collect();
    idx.sort_by_key(|&i| {
        let m = &metas[i];
        let rank = if m.class == 0 {
            0u8 // control: tiny, high network value
        } else if m.dest == *peer {
            1 // direct delivery
        } else {
            2 // spray
        };
        (rank, m.priority, m.size, m.remaining_ms(now_ms))
    });
    idx
}

/// Eviction order under quota pressure (doc 04 §2): receipt-confirmed first,
/// then by (lowest priority, largest, least remaining lifetime). Own
/// undelivered originals are NEVER evicted before expiry (I4).
///
/// Keys on `effective_priority`, not the declared byte — this is exactly the
/// "what a node stores prominently" local-policy decision doc 18 §5.2
/// reserves for the clamp (unlike `transfer_order` above, which must not).
pub fn eviction_order(
    metas: &[BundleMeta],
    signal: impl Fn(&Id16) -> SuppressSignal,
    now_ms: u64,
) -> Vec<Id16> {
    let mut candidates: Vec<&BundleMeta> = metas.iter().filter(|m| !m.own_origin).collect();
    candidates.sort_by_key(|m| {
        let confirmed = matches!(signal(&m.id), SuppressSignal::Purge);
        (
            core::cmp::Reverse(confirmed as u8), // receipt-confirmed first
            core::cmp::Reverse(m.effective_priority), // lowest effective priority first
            core::cmp::Reverse(m.size),          // largest first
            m.remaining_ms(now_ms),              // least life first
        )
    });
    candidates.into_iter().map(|m| m.id).collect()
}

#[cfg(test)]
mod tests;
