//! mmp-tal-btleplug — PC BLE TAL (Linux/Windows/macOS) via `btleplug`, central
//! role only (HANDOFF M5 — "first real-radio contact").
//!
//! This crate sits ABOVE the sans-IO protocol core (doc 12 §1): it owns no
//! protocol truth, only the mechanical radio work. `tools/layer-check` only
//! polices the seven crates below `mmp-node`, so `tokio` is allowed here.
//!
//! ## Scope: central role only (decision, not an oversight)
//!
//! `btleplug` is a central-role library; running a GATT *server*
//! (peripheral role) from a PC is platform-specific and out of scope for
//! this milestone. Concretely:
//!
//! - **PC ↔ phone** works today: phones run the GATT server (see
//!   `MOBILE_INTEGRATION.md`), and this crate dials them as a central.
//! - **PC ↔ PC** needs one side to be a peripheral, which this crate does not
//!   implement (BlueZ peripheral support exists but is a separate, deliberately
//!   deferred effort). Tracked as HANDOFF **M5b**.
//!
//! ## How the four steps in the original stub map to this implementation
//!
//! 1. **Beacon plane.** [`BleCentral::start_scan`] filters on
//!    [`DEV_SERVICE_UUID`]; a background task subscribed to the adapter's
//!    event stream turns `ServiceDataAdvertisement` events carrying that UUID
//!    into [`TalEvent::Beacon`] (payload bytes + RSSI, doc 09 §3.1). This
//!    crate does **not** parse the payload — `mmp-node` owns beacon parsing
//!    (doc 04); we only shuttle bytes. `ManufacturerDataAdvertisement` is not
//!    handled: doc 09 §2/§3.1 defines the MMP beacon as *Service Data* (AD
//!    type 0x16), not manufacturer data, so there is no "our service UUID"
//!    to match inside a manufacturer-data payload (which is keyed by a 16-bit
//!    manufacturer ID, not a UUID).
//! 2. **Dial (central role).** [`BleCentral::dial`] connects, discovers
//!    services, locates the TX/RX characteristics (see "characteristic UUIDs"
//!    below), subscribes to RX notifications, mints a [`LinkId`], and emits
//!    `TalEvent::LinkUp { initiator: true, .. }` once the subscription
//!    succeeds.
//! 3. **Shuttle.** A per-link task forwards each RX notification as
//!    `TalEvent::LinkData`; [`BleCentral::send`] drains caller-provided bytes
//!    into Write-Without-Response chunks of the link's negotiated chunk size
//!    (see "chunk size" below). Chunking itself is the pure, hardware-free
//!    [`framing::chunks`].
//! 4. **Peripheral role.** Out of scope, see above (M5b).
//!
//! ## Characteristic UUIDs (resolved: spec doc 09 does not pin them)
//!
//! Doc 09 §5.1 names a TX (Write Without Response) and RX (Notify)
//! characteristic but does not assign UUIDs to them (unlike the service UUID
//! in §2, which is pinned as a development placeholder pending SIG
//! assignment, OQ-12). [`DEV_TX_CHARACTERISTIC_UUID`] and
//! [`DEV_RX_CHARACTERISTIC_UUID`] are therefore development placeholders
//! derived from the same `6d6d7031-....-4d4d-5031-6d6573686d6c` service UUID
//! pattern, distinguished by the second UUID field (`0000` → `0001`/`0002`).
//! **They must match whatever the mobile GATT server (MOBILE_INTEGRATION.md)
//! publishes**; that doc does not yet pin concrete values either, so
//! reconciling the two is a prerequisite of the first real PC↔phone contact,
//! not a detail this crate can settle unilaterally.
//!
//! ## Chunk size / MTU (resolved: btleplug 0.11 exposes no portable MTU query)
//!
//! Checked against the `btleplug` 0.11.7 `api` surface (`Central`,
//! `Peripheral`, `PeripheralProperties`): none of it exposes a negotiated ATT
//! MTU, on any backend. Every link therefore uses the conservative
//! [`DEFAULT_CHUNK`] (23-byte ATT MTU floor − 3-byte ATT header = 20 bytes)
//! for its whole lifetime; the 247-byte/244-byte target from doc 09 §4 is
//! aspirational until a portable MTU signal exists (or platform-specific
//! escape hatches are added per backend — future work, not this milestone).
//! `chunk_size` is stored per-link (not as a crate-wide constant) so that
//! future per-platform MTU discovery only needs to change one field's
//! initial value, not the call sites that use it.
//!
//! ## Runtime discipline
//!
//! This crate owns no runtime. Every `async fn` here must run on a caller-
//! provided `tokio` runtime (`mmp-cli` will supply one); `BleCentral::new`
//! and `start_scan` — and everything else — must be called from within one.
//! Internally spawned tasks (`tokio::spawn`) reuse that same runtime.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use btleplug::api::{
    Central as _, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use uuid::Uuid;

pub use btleplug::platform::PeripheralId;
pub use mmp_tal::{
    LinkId, LinkQuality, TalEvent, DEV_RX_CHARACTERISTIC_UUID, DEV_SERVICE_UUID,
    DEV_TX_CHARACTERISTIC_UUID,
};

pub mod framing;

/// Conservative default per-link chunk size: 23-byte ATT MTU floor minus the
/// 3-byte ATT header. See module docs ("Chunk size / MTU") for why this
/// crate cannot currently raise it per-link.
const DEFAULT_CHUNK: usize = 20;

/// Maximum number of transport events waiting for the host event loop.
///
/// One maximum-size 4096-byte envelope takes `ceil((4096 + 3) / 20) = 205`
/// current-floor ATT notifications. Eight concurrently admitted sessions
/// therefore need 1640 queued data events for one envelope each; rounding to
/// 2048 establishes a deliberate ingress-memory threshold with link/control
/// and beacon headroom. It is not intended to absorb the entire possible
/// eight-frame credit burst; once full, producers await the host event loop
/// and the protocol's own CREDIT window continues to govern the wire rate.
/// The channel is count-bounded (not byte-bounded): a future larger negotiated
/// ATT MTU still has a finite controller bound, while producers await capacity
/// instead of dropping an encrypted chunk.
const EVENT_QUEUE_CAPACITY: usize = 2048;

fn event_channel() -> (mpsc::Sender<TalEvent>, mpsc::Receiver<TalEvent>) {
    mpsc::channel(EVENT_QUEUE_CAPACITY)
}

async fn acquire_write_gate(gate: &Arc<AsyncMutex<()>>) -> tokio::sync::OwnedMutexGuard<()> {
    gate.clone().lock_owned().await
}

/// A scanned or dial-able peer: `btleplug`'s peripheral identity plus the
/// most recent RSSI this crate observed for it (`None` if never seen with a
/// resolvable RSSI, e.g. the platform didn't report one).
#[derive(Debug, Clone)]
pub struct PeerHandle {
    pub id: PeripheralId,
    pub rssi: Option<i8>,
}

/// Everything that can go wrong dialing, sending on, or standing up a BLE
/// central. Wraps [`btleplug::Error`] for anything the underlying stack
/// reports; the other variants are conditions this crate itself detects.
#[derive(Debug)]
pub enum BleError {
    /// `Manager::adapters()` returned no Bluetooth adapter.
    NoAdapter,
    /// A `LinkId` was not found in this central's link table (never dialed,
    /// or already hung up / disconnected).
    UnknownLink,
    /// The peripheral for a link is not currently connected.
    NotConnected,
    /// `btleplug` itself reported an error.
    Btleplug(btleplug::Error),
}

impl fmt::Display for BleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BleError::NoAdapter => write!(f, "no BLE adapter available"),
            BleError::UnknownLink => write!(f, "unknown link"),
            BleError::NotConnected => write!(f, "not connected"),
            BleError::Btleplug(e) => write!(f, "btleplug error: {e}"),
        }
    }
}

impl std::error::Error for BleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BleError::Btleplug(e) => Some(e),
            BleError::NoAdapter | BleError::UnknownLink | BleError::NotConnected => None,
        }
    }
}

impl From<btleplug::Error> for BleError {
    fn from(e: btleplug::Error) -> Self {
        BleError::Btleplug(e)
    }
}

/// Per-link state held by [`BleCentral`] between `dial` and `hangup`/disconnect.
struct LinkState {
    peripheral: Peripheral,
    tx_characteristic: Characteristic,
    chunk_size: usize,
    write_gate: Arc<AsyncMutex<()>>,
}

/// A live BLE central: one Bluetooth adapter, a beacon/disconnect watcher
/// task, and a table of links this process dialed.
///
/// `new()` picks the first adapter reported by `btleplug`; a host with
/// several adapters and a preference among them needs a constructor this
/// crate does not yet provide (not needed for the M5 milestone: dev/test
/// hosts have exactly one).
pub struct BleCentral {
    adapter: Adapter,
    tx: mpsc::Sender<TalEvent>,
    // `std::sync::Mutex` (not tokio's) is correct here: it is only ever held
    // for the instant it takes to `Option::take`, never across an `.await`.
    events_rx: std::sync::Mutex<Option<mpsc::Receiver<TalEvent>>>,
    links: Arc<AsyncMutex<HashMap<LinkId, LinkState>>>,
}

impl BleCentral {
    /// Take the first Bluetooth adapter on the host and start watching it
    /// for beacons and disconnects. Must be called on a `tokio` runtime.
    pub async fn new() -> Result<Self, BleError> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .ok_or(BleError::NoAdapter)?;

        let (tx, rx) = event_channel();
        let links: Arc<AsyncMutex<HashMap<LinkId, LinkState>>> =
            Arc::new(AsyncMutex::new(HashMap::new()));

        tokio::spawn(watch_adapter(adapter.clone(), tx.clone(), links.clone()));

        Ok(BleCentral {
            adapter,
            tx,
            events_rx: std::sync::Mutex::new(Some(rx)),
            links,
        })
    }

    /// Take ownership of this central's event stream. There is exactly one
    /// receiver per `BleCentral`; calling this a second time panics — hold
    /// on to the first receiver instead of calling this again. The receiver
    /// must be drained while the central is active: radio producers apply
    /// bounded backpressure rather than dropping ordered link bytes.
    pub fn events(&self) -> mpsc::Receiver<TalEvent> {
        self.events_rx
            .lock()
            .expect("events_rx mutex poisoned")
            .take()
            .expect(
                "BleCentral::events() called more than once; only one receiver is ever handed out",
            )
    }

    /// Start scanning, filtered to [`DEV_SERVICE_UUID`]. Sightings surface as
    /// `TalEvent::Beacon` on the [`Self::events`] stream.
    pub async fn start_scan(&self) -> Result<(), BleError> {
        self.adapter
            .start_scan(ScanFilter { services: vec![service_uuid()] })
            .await?;
        Ok(())
    }

    /// List peripherals the adapter has already observed (via `start_scan`)
    /// as dial-able [`PeerHandle`]s.
    ///
    /// Not part of the milestone's minimum API, but included because without
    /// it `dial` has no way to obtain a `PeerHandle`: this crate forwards raw
    /// beacon bytes (point 1 above) without parsing them, so it cannot itself
    /// correlate "the peer `mmp-node` decided to dial" with a `PeripheralId`.
    /// The caller (`mmp-node`/`mmp-cli`) does that correlation, using this
    /// list plus whatever it parsed from the beacon payload.
    pub async fn known_peers(&self) -> Result<Vec<PeerHandle>, BleError> {
        let mut peers = Vec::new();
        for p in self.adapter.peripherals().await? {
            let rssi = p
                .properties()
                .await
                .ok()
                .flatten()
                .and_then(|props| props.rssi)
                .map(clamp_rssi);
            peers.push(PeerHandle { id: p.id(), rssi });
        }
        Ok(peers)
    }

    /// Connect to `peer`, discover the MMP GATT service, subscribe to RX
    /// notifications, and mint a [`LinkId`] for it (see module docs for the
    /// characteristic-UUID placeholders). Schedules `TalEvent::LinkUp` on
    /// success before that link's RX notifications; callers must drain
    /// [`Self::events`] while the central is active.
    pub async fn dial(&self, peer: PeerHandle) -> Result<LinkId, BleError> {
        let link = mint_link_id(&peer.id);
        let peripheral = self.adapter.peripheral(&peer.id).await?;
        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let characteristics = peripheral.characteristics();
        let tx_characteristic = characteristics
            .iter()
            .find(|c| c.uuid == tx_characteristic_uuid())
            .cloned()
            .ok_or(btleplug::Error::NoSuchCharacteristic)?;
        let rx_characteristic = characteristics
            .iter()
            .find(|c| c.uuid == rx_characteristic_uuid())
            .cloned()
            .ok_or(btleplug::Error::NoSuchCharacteristic)?;

        peripheral.subscribe(&rx_characteristic).await?;
        let mut notifications = peripheral.notifications().await?;

        self.links.lock().await.insert(
            link,
            LinkState {
                peripheral,
                tx_characteristic,
                chunk_size: DEFAULT_CHUNK,
                write_gate: Arc::new(AsyncMutex::new(())),
            },
        );

        let quality = LinkQuality { mtu: (DEFAULT_CHUNK + 3) as u16, ..LinkQuality::default() };
        let data_tx = self.tx.clone();
        let rx_uuid = rx_characteristic.uuid;
        tokio::spawn(async move {
            // Keep LinkUp ahead of every RX notification for this link while
            // allowing dial() to return even when the shared event queue is
            // full; the host event loop must drain events to release this
            // bounded backpressure.
            if data_tx
                .send(TalEvent::LinkUp { link, initiator: true, quality })
                .await
                .is_err()
            {
                return;
            }
            while let Some(notification) = notifications.next().await {
                if notification.uuid == rx_uuid
                    && data_tx
                        .send(TalEvent::LinkData { link, bytes: notification.value })
                        .await
                        .is_err()
                {
                    // Owner dropped the event receiver; nobody is listening.
                    break;
                }
            }
        });

        Ok(link)
    }

    /// Chunk `bytes` into Write-Without-Response ATT payloads and send them,
    /// in order, on `link`'s TX characteristic.
    pub async fn send(&self, link: LinkId, bytes: &[u8]) -> Result<(), BleError> {
        let (peripheral, tx_characteristic, chunk_size, write_gate) = {
            let links = self.links.lock().await;
            let state = links.get(&link).ok_or(BleError::UnknownLink)?;
            (
                state.peripheral.clone(),
                state.tx_characteristic.clone(),
                state.chunk_size,
                state.write_gate.clone(),
            )
        };

        let _write_guard = acquire_write_gate(&write_gate).await;
        if !peripheral.is_connected().await? {
            return Err(BleError::NotConnected);
        }

        for chunk in framing::chunks(bytes, chunk_size) {
            peripheral
                .write(&tx_characteristic, chunk, WriteType::WithoutResponse)
                .await?;
        }
        Ok(())
    }

    /// Disconnect `link` (best-effort — errors are swallowed, matching the
    /// "all disconnect reasons are alike" stance in `TalEvent::LinkDown`'s
    /// docs) and emit `TalEvent::LinkDown` for it. A no-op (still emits
    /// `LinkDown`) if `link` is already gone from the table.
    pub async fn hangup(&self, link: LinkId) {
        let write_gate = self
            .links
            .lock()
            .await
            .get(&link)
            .map(|state| state.write_gate.clone());
        let _write_guard = match write_gate {
            Some(gate) => Some(acquire_write_gate(&gate).await),
            None => None,
        };
        let state = self.links.lock().await.remove(&link);
        if let Some(state) = state {
            let _ = state.peripheral.disconnect().await;
        }
        let _ = self.tx.send(TalEvent::LinkDown { link }).await;
    }
}

/// Background task: turns the adapter's `CentralEvent` stream into
/// `TalEvent::Beacon` (on Service Data ads matching our UUID) and
/// `TalEvent::LinkDown` (on disconnects of links we still track). Exits when
/// the adapter's event stream ends or nobody is listening on `tx` anymore.
async fn watch_adapter(
    adapter: Adapter,
    tx: mpsc::Sender<TalEvent>,
    links: Arc<AsyncMutex<HashMap<LinkId, LinkState>>>,
) {
    let uuid = service_uuid();
    let mut events = match adapter.events().await {
        Ok(stream) => stream,
        Err(_) => return,
    };

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                let Some(payload) = service_data.get(&uuid) else { continue };
                let rssi = match adapter.peripheral(&id).await {
                    Ok(p) => p
                        .properties()
                        .await
                        .ok()
                        .flatten()
                        .and_then(|props| props.rssi)
                        .map(clamp_rssi)
                        .unwrap_or(0),
                    Err(_) => 0,
                };
                if tx.send(TalEvent::Beacon { payload: payload.clone(), rssi }).await.is_err() {
                    return;
                }
            }
            CentralEvent::DeviceDisconnected(id) => {
                let link = mint_link_id(&id);
                let write_gate = links
                    .lock()
                    .await
                    .get(&link)
                    .map(|state| state.write_gate.clone());
                let _write_guard = match write_gate {
                    Some(gate) => Some(acquire_write_gate(&gate).await),
                    None => None,
                };
                let removed = links.lock().await.remove(&link).is_some();
                if removed && tx.send(TalEvent::LinkDown { link }).await.is_err() {
                    return;
                }
            }
            _ => {}
        }
    }
}

fn clamp_rssi(rssi: i16) -> i8 {
    rssi.clamp(i8::MIN as i16, i8::MAX as i16) as i8
}

fn service_uuid() -> Uuid {
    DEV_SERVICE_UUID.parse().expect("DEV_SERVICE_UUID is a valid UUID literal")
}

fn tx_characteristic_uuid() -> Uuid {
    DEV_TX_CHARACTERISTIC_UUID
        .parse()
        .expect("DEV_TX_CHARACTERISTIC_UUID is a valid UUID literal")
}

fn rx_characteristic_uuid() -> Uuid {
    DEV_RX_CHARACTERISTIC_UUID
        .parse()
        .expect("DEV_RX_CHARACTERISTIC_UUID is a valid UUID literal")
}

/// Mint a `LinkId` as the first 8 LE bytes of `blake3(Debug string of id)`.
/// Deterministic per peer (reconnects reuse the same `LinkId`, which is the
/// desired session-table behavior) and practically collision-free across
/// distinct peers. Generic over `Debug` rather than tied to
/// `btleplug::platform::PeripheralId` (whose constructor is private to that
/// crate) so it stays unit-testable without hardware.
fn mint_link_id(id: &impl fmt::Debug) -> LinkId {
    let digest = blake3::hash(format!("{id:?}").as_bytes());
    u64::from_le_bytes(digest.as_bytes()[..8].try_into().expect("blake3 digest is 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::{acquire_write_gate, event_channel, mint_link_id, EVENT_QUEUE_CAPACITY};
    use mmp_tal::TalEvent;
    use std::sync::Arc;
    use tokio::sync::Mutex as AsyncMutex;

    #[tokio::test]
    async fn write_gate_serializes_concurrent_senders() {
        let gate = Arc::new(AsyncMutex::new(()));
        let first = acquire_write_gate(&gate).await;
        let gate_for_second = gate.clone();
        let second = tokio::spawn(async move { acquire_write_gate(&gate_for_second).await });

        tokio::task::yield_now().await;
        assert!(!second.is_finished(), "a second sender must wait for the whole first write");

        drop(first);
        second.await.expect("second sender must acquire after the first releases");
    }

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

    #[test]
    fn mint_link_id_is_deterministic_for_the_same_peer() {
        assert_eq!(mint_link_id(&"peer-a"), mint_link_id(&"peer-a"));
    }

    #[test]
    fn mint_link_id_differs_across_distinct_peers() {
        assert_ne!(mint_link_id(&"peer-a"), mint_link_id(&"peer-b"));
    }
}
