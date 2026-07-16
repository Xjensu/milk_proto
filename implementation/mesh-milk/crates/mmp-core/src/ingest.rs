//! Bundle ingest pipeline (doc 03 §4.1.2, doc 11 §4) — the security-critical
//! hot path, in EXACTLY this order:
//!
//! decode-canonical → verify_strict (inline origin key, domain-separated) →
//! SeenIndex check → SeenIndex insert FROM FIRST SIGHT (AUDIT F1) →
//! lifetime clamp → admission → store.
//!
//! No unauthenticated input changes durable state; failures DROP, never
//! quarantine, never forward (AUDIT F2).

use mmp_types::bundle::{class, prio, Bundle};
use mmp_types::consts::class_max_lifetime_ms;
use mmp_types::domain::SigDomain;
use mmp_types::ids::NodeId;

use crate::seen::SeenIndex;
use crate::store::{BundleMeta, BundleStore, SlotState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    NonCanonical,
    BadSignature,
    Admission,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Stored; `to_self` = destined to this node (deliver upstream too).
    Stored { to_self: bool },
    Duplicate,
    Dropped(DropReason),
}

/// Ingest one wire bundle. On `Stored`, the decoded bundle is returned so the
/// node can run the class-specific delivery path without re-decoding.
pub fn ingest(
    bytes: &[u8],
    now_ms: u64,
    self_node: &NodeId,
    seen: &mut SeenIndex,
    store: &mut dyn BundleStore,
    admission: &mut dyn FnMut(&Bundle, &NodeId) -> bool,
) -> (Disposition, Option<Bundle>) {
    // 1. Canonical decode — reject non-canonical (doc 10 §6).
    let Ok(bundle) = Bundle::decode_canonical(bytes) else {
        return (Disposition::Dropped(DropReason::NonCanonical), None);
    };
    // 2. Derive, never trust: NodeID from the inline origin key.
    let origin_node = bundle.origin_node();
    // 3. Unconditional verify-before-store (strict, domain-separated).
    if mmp_crypto::verify(SigDomain::Bundle, &bundle.origin, &bundle.canonical_zeroed(), &bundle.signature)
        .is_err()
    {
        return (Disposition::Dropped(DropReason::BadSignature), None);
    }
    // 4. Resurrection-resistant dedup.
    let id = bundle.id();
    if seen.contains(&id) {
        return (Disposition::Duplicate, None);
    }
    // 5. SeenIndex FROM FIRST SIGHT, for class-max lifetime (AUDIT F1) —
    //    before any admission decision, so even a dropped bundle can't loop.
    seen.insert(&id, bundle.class, now_ms);
    // 6. Receive-side lifetime clamp (doc 03 §4.3). Silent, never an error.
    let lifetime = bundle.lifetime_ms.min(class_max_lifetime_ms(bundle.class));
    if lifetime == 0 {
        return (Disposition::Dropped(DropReason::Expired), None);
    }
    // 7. Admission: quota, blocklist, per-origin bucket, tier rules (doc 05).
    if !admission(&bundle, &origin_node) {
        return (Disposition::Dropped(DropReason::Admission), None);
    }
    // 8. Durable only now.
    let to_self = bundle.dest == *self_node;
    let meta = BundleMeta {
        id,
        class: bundle.class,
        origin: origin_node,
        dest: bundle.dest,
        priority: bundle.priority,
        // Doc 18 §5.2 default clamp: unauthenticated (or not-yet-evaluated)
        // class-5 bundles never get better than NORMAL local prominence. This
        // is the fail-safe half; elevation is a post-store step once an
        // AuthCert/bucket check passes (mmp-node::handle_announcement) — it
        // can only ever raise this back toward the declared byte, never lower
        // priority for any other class, and it never causes a drop here.
        effective_priority: if bundle.class == class::ANNOUNCEMENT {
            bundle.priority.max(prio::NORMAL)
        } else {
            bundle.priority
        },
        size: bytes.len() as u32,
        stored_at: now_ms,
        expires_at: now_ms + lifetime,
        budget: 1,
        own_origin: origin_node == *self_node,
        slot: SlotState::Stored,
    };
    store.put(meta, bytes);
    (Disposition::Stored { to_self }, Some(bundle))
}
