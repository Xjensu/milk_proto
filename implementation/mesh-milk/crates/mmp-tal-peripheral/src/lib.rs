//! mmp-tal-peripheral — PC BLE **peripheral** role: GATT server + advertiser
//! (HANDOFF M5b; handbook §4.1).
//!
//! `mmp-tal-btleplug` is central-only — `btleplug` cannot advertise or run a
//! GATT server on any OS. For PC↔PC contact (or a PC a phone can dial) one
//! side must be a peripheral, and PC peripheral APIs are inherently
//! platform-specific. This crate provides [`PeripheralRole`], the mirror
//! image of `BleCentral`, speaking the same [`TalEvent`] contract to the rest
//! of the system and the same dev GATT layout over the air.
//!
//! ## Design decisions (M5b)
//!
//! 1. **Separate crate, not a feature of `mmp-tal-btleplug`.** The central
//!    crate declares `btleplug`+`tokio`+`futures` unconditionally; peripheral
//!    backends need per-OS stacks (`bluer`→D-Bus→libdbus on Linux, WinRT via
//!    the `windows` crate on Windows) that are heavier and would turn the
//!    central crate's manifest into a cfg/feature lattice. A host that only
//!    ever dials should not build a D-Bus stack. See `Cargo.toml` for the
//!    cfg(target_os)-not-features rationale.
//! 2. **No portable dual-role crate exists to adopt (re-verified 2026-07-16).**
//!    Survey: `bluest` documents "Peripheral and Server roles not supported";
//!    `ble-peripheral-rust` is cross-platform in intent but pre-1.0 with ~60
//!    stars, 13 commits and no releases — not a foundation for a reference
//!    implementation; `burble` bypasses the OS stack entirely (raw USB HCI,
//!    needs a dedicated controller); `gattrs` is Linux-only. Hand-rolling
//!    per-OS against first-party stacks (BlueZ, WinRT) remains the sound call.
//! 3. **Platform scope: Linux/BlueZ (`bluer`) is implemented; Windows and
//!    macOS are explicit stubs.** Linux first because BlueZ is scriptable and
//!    testable in CI-adjacent environments (and `bluer` is the official BlueZ
//!    binding). The Windows stub carries a concrete WinRT design sketch in
//!    `src/unsupported.rs`; macOS is lowest priority per the handbook.
//! 4. **GATT role inversion** — the single easiest thing to get backwards:
//!    the central crate's "TX" characteristic (`…-0001-…`) is the one the
//!    central *writes*; as the peripheral we therefore expose it as
//!    Write-Without-Response and treat every write as **inbound**
//!    [`TalEvent::LinkData`]. The central crate's "RX" (`…-0002-…`) is the one
//!    the central *subscribes* to; we expose it as Notify and push **outbound**
//!    bytes through it in [`PeripheralRole::notify`]. The constants below are
//!    deliberately named from the central's perspective
//!    (`DEV_CENTRAL_TX_…`/`DEV_CENTRAL_RX_…`) so a reader is forced through
//!    the inversion once instead of trusting an ambiguous local "TX"/"RX".
//! 5. **LinkId minting** uses the identical recipe as the central crate
//!    (first 8 LE bytes of `blake3(Debug string of the OS peer identifier)`).
//!    Honesty note: the central mints from `btleplug::PeripheralId` and this
//!    crate mints from `bluer::Address` — different Debug strings for the
//!    same physical peer — so cross-role ID *equality* is not achievable;
//!    what is shared is the derivation recipe (and its determinism per peer,
//!    which is what the session table relies on).
//! 6. **Chunk size: real negotiated MTU, floor 20.** Unlike btleplug 0.11
//!    (no portable MTU query, so the central pins the 20-byte floor for a
//!    link's lifetime), BlueZ reports the exchanged ATT MTU on
//!    `AcquireWrite`/`AcquireNotify`, surfaced by `bluer` as
//!    `CharacteristicReader::mtu()`/`CharacteristicWriter::mtu()`. We chunk
//!    outbound notifications to `max(mtu − 3, 20)` (doc 09 §5.1's (MTU−3)
//!    ATT payload rule; 3 = ATT opcode+handle overhead — subtracted here
//!    because BlueZ reports the raw exchanged MTU, and losing ≤3 B per
//!    notification is cheaper than risking a too-large packet if a bluer
//!    version ever bounds `send()` at the usable payload instead). Follow-up
//!    (not done here, per M5b scope): teach the central crate per-platform
//!    MTU escape hatches so the write path enjoys the same lift.
//!
//! ## Contract (same as the central crate)
//!
//! - Emits [`TalEvent`]s on a bounded FIFO channel ([`PeripheralRole::events`],
//!   take-once). The 2048-event queue applies async backpressure to ATT read
//!   and link-control producers rather than dropping ordered encrypted bytes.
//!   `LinkUp` carries `initiator: false` — the central dialed us.
//! - Accepts outbound bytes via [`PeripheralRole::notify`], chunked.
//! - Owns no runtime: every `async fn` runs on the caller's tokio runtime;
//!   internal tasks are `tokio::spawn`ed onto it.
//! - Shuttles bytes only: beacon payloads are NOT constructed or parsed here
//!   (`mmp-node` owns that, doc 04); [`PeripheralRole::advertise`] takes the
//!   ready-made payload and the host re-calls it each beacon slot
//!   ([`readvertise_due`]).
//!
//! `tools/layer-check` polices only the crates below `mmp-node`, so tokio is
//! fine here (this crate sits beside `mmp-tal-btleplug`, above the core).

use core::fmt;
#[cfg(any(target_os = "linux", test))]
use tokio::sync::mpsc;

pub use mmp_tal::{LinkId, LinkQuality, TalEvent};
use mmp_types::consts::EPHEM_SLOT_MS;

pub mod framing;

/// Maximum number of transport events waiting for the host event loop.
///
/// The value mirrors the central TAL: eight concurrently admitted sessions
/// need 1640 twenty-byte notifications to hold one maximum-size 4096-byte
/// envelope per session; 2048 is a deliberate ingress-memory threshold with
/// control and beacon headroom, not a promise to absorb an entire eight-frame
/// credit burst. Producers await capacity, so a full queue applies
/// backpressure instead of dropping an ordered encrypted ATT write.
#[cfg(any(target_os = "linux", test))]
pub(crate) const EVENT_QUEUE_CAPACITY: usize = 2048;

#[cfg(any(target_os = "linux", test))]
pub(crate) fn event_channel() -> (mpsc::Sender<TalEvent>, mpsc::Receiver<TalEvent>) {
    mpsc::channel(EVENT_QUEUE_CAPACITY)
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::PeripheralRole;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::PeripheralRole;

/// Single source of truth for the dev service UUID (E3-S3): re-exported from
/// `mmp-tal` rather than a separate literal, so this crate and the central
/// (`mmp-tal-btleplug`) can never drift. A SIG-assigned 16-bit UUID is a
/// production release blocker (OQ-12); doc 09 §2 pins only this service UUID,
/// not the characteristic UUIDs.
pub use mmp_tal::DEV_SERVICE_UUID;

/// The characteristic the central crate calls "TX" (`mmp_tal::DEV_TX_CHARACTERISTIC_UUID`):
/// the central WRITES here (Write Without Response). **For this peripheral it
/// is the INBOUND path** — each write we accept becomes [`TalEvent::LinkData`].
/// Named from the central's perspective to make the role inversion explicit;
/// same underlying UUID, single source of truth (E3-S3).
pub const DEV_CENTRAL_TX_CHARACTERISTIC_UUID: &str = mmp_tal::DEV_TX_CHARACTERISTIC_UUID;

/// The characteristic the central crate calls "RX" (`mmp_tal::DEV_RX_CHARACTERISTIC_UUID`):
/// the central SUBSCRIBES here (Notify). **For this peripheral it is the
/// OUTBOUND path** — [`PeripheralRole::notify`] pushes chunks through it.
pub const DEV_CENTRAL_RX_CHARACTERISTIC_UUID: &str = mmp_tal::DEV_RX_CHARACTERISTIC_UUID;

/// Everything that can go wrong advertising, serving, or notifying.
///
/// `#[non_exhaustive]` because the variant set is platform-dependent (the
/// `Bluer` variant only exists on Linux); cross-platform callers must keep a
/// catch-all arm anyway.
#[derive(Debug)]
#[non_exhaustive]
pub enum PeriphError {
    /// A `LinkId` was not found in this peripheral's link table (central
    /// never connected, or already disconnected).
    UnknownLink,
    /// The central is connected (it wrote to us) but has not subscribed to
    /// the notify characteristic yet, so there is no outbound path.
    NotSubscribed,
    /// This build's OS has no peripheral backend (Windows/macOS stubs); the
    /// payload says what implementing one would take.
    Unsupported(&'static str),
    /// The BlueZ stack reported an error.
    #[cfg(target_os = "linux")]
    Bluer(bluer::Error),
}

impl fmt::Display for PeriphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeriphError::UnknownLink => write!(f, "unknown link"),
            PeriphError::NotSubscribed => write!(f, "central has not subscribed to notifications"),
            PeriphError::Unsupported(what) => write!(f, "peripheral role unsupported: {what}"),
            #[cfg(target_os = "linux")]
            PeriphError::Bluer(e) => write!(f, "bluer error: {e}"),
        }
    }
}

impl std::error::Error for PeriphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(target_os = "linux")]
            PeriphError::Bluer(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(target_os = "linux")]
impl From<bluer::Error> for PeriphError {
    fn from(e: bluer::Error) -> Self {
        PeriphError::Bluer(e)
    }
}

/// True when the host should re-call [`PeripheralRole::advertise`] with a
/// fresh `Node::beacon()` payload: the beacon rotates per 15-minute
/// `EPHEM_SLOT_MS` slot (doc 09 §3.2), and `mmp-node` keys the beacon content
/// on `now_ms / EPHEM_SLOT_MS` — so this is slot-boundary-crossing arithmetic
/// (not "elapsed ≥ 15 min"), keeping the advertisement aligned with the exact
/// payload the node would mint for the current slot.
///
/// `last_advertised_ms` is when the host last called `advertise`; monotone
/// milliseconds, same clock the host feeds `Node::tick`.
pub fn readvertise_due(last_advertised_ms: u64, now_ms: u64) -> bool {
    now_ms / EPHEM_SLOT_MS != last_advertised_ms / EPHEM_SLOT_MS
}

/// Mint a `LinkId` as the first 8 LE bytes of `blake3(Debug string of id)` —
/// the SAME recipe as `mmp-tal-btleplug` (its `mint_link_id`), kept in
/// lockstep by convention: deterministic per peer (a reconnecting central
/// reuses its `LinkId`) and practically collision-free across peers. See
/// module docs (decision 5) for why cross-role ID equality is still not a
/// thing. Generic over `Debug` so it stays unit-testable without hardware.
// Only the cfg(linux) backend calls this outside of tests; keep non-Linux
// `cargo build` warning-free without hiding real dead code on Linux.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn mint_link_id(id: &impl fmt::Debug) -> LinkId {
    let digest = blake3::hash(format!("{id:?}").as_bytes());
    u64::from_le_bytes(digest.as_bytes()[..8].try_into().expect("blake3 digest is 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_channel_applies_bounded_fifo_backpressure() {
        let (tx, mut rx) = event_channel();
        for sequence in 0..EVENT_QUEUE_CAPACITY {
            tx.send(TalEvent::LinkData { link: 7, bytes: vec![sequence as u8] })
                .await
                .expect("capacity-sized event burst must fit");
        }

        assert!(tx.try_send(TalEvent::LinkData { link: 7, bytes: vec![0xff] }).is_err());

        let producer = tokio::spawn(async move {
            tx.send(TalEvent::LinkData { link: 7, bytes: vec![0xfe] })
                .await
                .expect("producer must resume after the receiver drains");
        });
        tokio::task::yield_now().await;
        assert!(!producer.is_finished(), "full channel must apply backpressure");

        for sequence in 0..EVENT_QUEUE_CAPACITY {
            let event = rx.recv().await.expect("queued event must remain available");
            assert_eq!(event, TalEvent::LinkData { link: 7, bytes: vec![sequence as u8] });
        }
        producer.await.expect("producer task must complete");
        assert_eq!(
            rx.recv().await,
            Some(TalEvent::LinkData { link: 7, bytes: vec![0xfe] })
        );
    }

    /// The three constants must also be *parseable* UUIDs (the Linux backend
    /// parses them at startup; catch a typo on every platform's test run).
    #[test]
    fn gatt_uuids_parse() {
        for s in [
            DEV_SERVICE_UUID,
            DEV_CENTRAL_TX_CHARACTERISTIC_UUID,
            DEV_CENTRAL_RX_CHARACTERISTIC_UUID,
        ] {
            s.parse::<uuid::Uuid>().expect("dev UUID constant must parse");
        }
    }

    #[test]
    fn mint_link_id_is_deterministic_for_the_same_peer() {
        assert_eq!(mint_link_id(&"aa:bb:cc:dd:ee:ff"), mint_link_id(&"aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn mint_link_id_differs_across_distinct_peers() {
        assert_ne!(mint_link_id(&"aa:bb:cc:dd:ee:ff"), mint_link_id(&"aa:bb:cc:dd:ee:00"));
    }

    /// Recipe pin: blake3 of the Debug string, first 8 bytes LE. If either
    /// crate ever drifts from the shared recipe, this vector breaks loudly.
    #[test]
    fn mint_link_id_recipe_is_blake3_of_debug_first_8_le() {
        let id = "peer";
        let digest = blake3::hash(format!("{id:?}").as_bytes());
        let expect = u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap());
        assert_eq!(mint_link_id(&id), expect);
    }

    #[test]
    fn readvertise_not_due_within_the_same_slot() {
        assert!(!readvertise_due(0, EPHEM_SLOT_MS - 1));
        assert!(!readvertise_due(EPHEM_SLOT_MS, EPHEM_SLOT_MS + 1));
    }

    #[test]
    fn readvertise_due_when_the_slot_boundary_is_crossed() {
        assert!(readvertise_due(EPHEM_SLOT_MS - 1, EPHEM_SLOT_MS));
        assert!(readvertise_due(0, 3 * EPHEM_SLOT_MS));
    }
}
