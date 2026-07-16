//! Protocol constants — the single source module (HANDOFF §4, doc 03 §9).
//!
//! Everything marked ⚠ in the specs is provisional until the doc-13 de-⚠ pass;
//! keeping them all here makes that pass a one-file change.

/// Maximum encoded bundle size, bytes (doc 03 §4.1.1). ⚠
pub const MAX_BUNDLE: usize = 4096;
/// Maximum session-frame payload, bytes (doc 10 §2); shares the bundle cap. ⚠
pub const MAX_FRAME_PAYLOAD: usize = 4096;
/// Copy-budget ceiling; grants are clamped to this on receive (doc 05 §2.3.1, AUDIT F3). ⚠
pub const L_MAX: u8 = 32;

const HOUR_MS: u64 = 3_600_000;
/// Class-0 control lifetime (receipts, acks): 6 h. ⚠
pub const LIFETIME_CONTROL_MS: u64 = 6 * HOUR_MS;
/// Class-1/2 message and class-4 symbol lifetime: 72 h. ⚠
pub const LIFETIME_MESSAGE_MS: u64 = 72 * HOUR_MS;
/// Class-3 claim lifetime: 7 d. ⚠
pub const LIFETIME_CLAIM_MS: u64 = 7 * 24 * HOUR_MS;

/// Receive-side lifetime clamp per class (doc 03 §4.3). Unknown classes get the
/// message maximum (store-and-forward rule, doc 03 §8).
pub const fn class_max_lifetime_ms(class: u8) -> u64 {
    match class {
        0 => LIFETIME_CONTROL_MS,
        3 => LIFETIME_CLAIM_MS,
        _ => LIFETIME_MESSAGE_MS,
    }
}

/// Store quotas, bytes (doc 03 §9). ⚠
pub const QUOTA_STANDARD: u64 = 128 * 1024 * 1024;
pub const QUOTA_CORE: u64 = 32 * 1024;
pub const QUOTA_ANCHOR: u64 = 2 * 1024 * 1024 * 1024;

/// RBSR terminal-list threshold and branching factor (doc 06 §3.2). ⚠
pub const RBSR_T_LIST: usize = 24;
pub const RBSR_B_F: usize = 16;
/// Items with less remaining lifetime than this are excluded from offers (doc 06 §2). ⚠
pub const OFFER_MIN_LIFETIME_MS: u64 = 10 * 60 * 1000;

/// Spray/utility constants (doc 05). ⚠
pub const THETA_SPRAY: f32 = 0.3;
pub const THETA_FOCUS: f32 = 0.15;
pub const SIGMA_TIE: f32 = 0.5;
pub const P_INIT: f32 = 0.5;
pub const GAMMA_PER_HOUR: f32 = 0.98;
/// Receipt spray budget and lifetime (doc 05 §7.1). ⚠
pub const L_RECEIPT: u8 = 8;
/// P-table and ContactGraph caps — the 10k-survivability answer (doc 05 §4). ⚠
pub const P_TABLE_CAP: usize = 256;
pub const CONTACT_GRAPH_CAP: usize = 4096;

/// Per-origin token bucket (doc 05 §8): rate/h, burst, WoT multiplier. ⚠
pub const ORIGIN_BUCKET_RATE_PER_H: u64 = 32 * 1024;
pub const ORIGIN_BUCKET_BURST: u64 = 128 * 1024;
pub const WOT_MULTIPLIER: u64 = 4;
/// In-flight cap per (origin → dest) as app backpressure (doc 05 §8). ⚠
pub const IN_FLIGHT_CAP: usize = 64;

/// ReceiptFilter / SeenIndex bucketing: 12 × 6 h covers the 72 h class max (doc 06 §6.1). ⚠
pub const FILTER_BUCKETS: usize = 12;
pub const FILTER_BUCKET_MS: u64 = 6 * HOUR_MS;
/// Standard-tier Bloom capacity per bucket and bits/element for ≈1 % FP (k = 7). ⚠
pub const BLOOM_CAPACITY: usize = 8192;
pub const BLOOM_BITS_PER_ELEMENT: usize = 10;
pub const BLOOM_K: usize = 7;
/// Wire cap per SUMMARY bloom bucket (mini-filter, doc 06 §9): the full
/// Standard bucket (~10 KiB) exceeds MAX_FRAME_PAYLOAD, so buckets ship
/// OR-folded to this size — a pure Hint, so extra FPs are harmless (AUDIT F11).
pub const BLOOM_SUMMARY_MAX: usize = 1280;

/// Conversation-ratchet skipped-key window (doc 08 §5.1); Core tier uses 32. ⚠
pub const RATCHET_WINDOW: u32 = 512;
/// One-time prekeys per batch claim (doc 08 §4.2). ⚠
pub const PREKEY_BATCH: usize = 16;

/// Session timers, ms (doc 11 §7). ⚠
pub const HANDSHAKE_BUDGET_MS: u64 = 5_000;
pub const SESSION_IDLE_MS: u64 = 10_000;
pub const SESSION_AIRTIME_CAP_MS: u64 = 60_000;
pub const REDIAL_SUPPRESS_MS: u64 = 60_000;
pub const BACKOFF_BASE_MS: u64 = 1_000;
pub const BACKOFF_CAP_MS: u64 = 5 * 60 * 1000;
/// Ephemeral-ID / RPA rotation slot (doc 09 §3.2). ⚠
pub const EPHEM_SLOT_MS: u64 = 15 * 60 * 1000;

/// File-transfer caps (doc 16). ⚠
pub const FILE_MAX: u64 = 8 * 1024 * 1024;
pub const ALBUM_MAX_FILES: usize = 16;
pub const THUMB_MAX: usize = 32 * 1024;
pub const NAME_MAX: usize = 255;
pub const MIME_MAX: usize = 64;
/// Nickname claim body cap (doc 06 §4.1). ⚠
pub const NICKNAME_MAX: usize = 32;

/// Authority/announcement caps (doc 18). ⚠
/// Cap on entries per REVOCATION_LIST update (doc 18 §6) — overflow means the
/// issuer should rotate its root instead of growing the list further.
pub const REVLIST_MAX: usize = 48;
/// Clock-skew tolerance for `not_after` checks against the receiver's own
/// local clock — no shared clock in a DTN (doc 18 §6, CR-A6).
pub const NOT_AFTER_SKEW_S: u32 = 15 * 60;
/// Local decode-safety bound on announcement text length. Doc 18 §3 does not
/// cap `text` itself; this is local policy (MAX_BUNDLE headroom), not protocol.
pub const ANNOUNCE_TEXT_MAX: usize = 2048;
