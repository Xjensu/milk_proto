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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use btleplug::api::{
    Central as _, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

pub use btleplug::platform::PeripheralId;
pub use mmp_tal::{
    LinkId, LinkQuality, TalEvent, DEV_RX_CHARACTERISTIC_UUID, DEV_SERVICE_UUID,
    DEV_TX_CHARACTERISTIC_UUID,
};

pub mod framing;
mod event_queue;
use event_queue::{ControlEnqueueError, DataEnqueueError, EventSender};

/// Conservative default per-link chunk size: 23-byte ATT MTU floor minus the
/// 3-byte ATT header. See module docs ("Chunk size / MTU") for why this
/// crate cannot currently raise it per-link.
const DEFAULT_CHUNK: usize = 20;

/// Bounded transport-ingress sizing.
///
/// One maximum-size 4096-byte envelope takes `ceil((4096 + 3) / 20) = 205`
/// current-floor ATT notifications. Eight concurrently admitted sessions
/// therefore need 1640 queued data events for one envelope each; rounding to
/// 2048 establishes a deliberate ingress-memory threshold. The internal bus
/// reserves 64 control records and exposes 1984 data slots. Producers never
/// await the host event loop: a full data lane retires only the affected link
/// (preserving stream ordering), while low-priority beacon data may be lost.
/// The public `events()` receiver remains a bounded Tokio FIFO fed by a
/// dispatcher, so a stalled consumer cannot hold lifecycle locks.
fn event_channel() -> (EventSender, mpsc::Receiver<TalEvent>) {
    event_queue::event_channel()
}

fn generation_is_current(current: Option<u64>, expected: u64) -> bool {
    current == Some(expected)
}

fn allocate_generation(counter: &AtomicU64) -> u64 {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1).unwrap_or(1);
        match counter.compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(observed) => current = observed,
        }
    }
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
    generation: u64,
}

type Links = Arc<AsyncMutex<HashMap<LinkId, LinkState>>>;

// Lifecycle transitions use the lock order event_gate -> write_gate -> links.
// This makes removal and its LinkDown enqueue one atomic transition with
// respect to reconnects, while send() deliberately uses write_gate -> links
// and never waits for event_gate. Lifecycle producers use synchronous
// reserved-control/data ingress; only the detached dispatcher awaits the
// public receiver, so reconnect gates never inherit host-consumer latency.

async fn emit_if_current(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &EventSender,
    link: LinkId,
    generation: u64,
    event: TalEvent,
) -> bool {
    let _event_guard = event_gate.lock().await;
    let current = links
        .lock()
        .await
        .get(&link)
        .map(|state| state.generation);
    if !generation_is_current(current, generation) {
        return false;
    }
    !matches!(tx.try_link_data(link, generation, event), Err(DataEnqueueError::Full | DataEnqueueError::Stale))
}

async fn emit_replacement(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &EventSender,
    link: LinkId,
    old_generation: Option<u64>,
    new_generation: u64,
    quality: LinkQuality,
) -> bool {
    let _event_guard = event_gate.lock().await;
    let current = links
        .lock()
        .await
        .get(&link)
        .map(|state| state.generation);
    if !generation_is_current(current, new_generation) {
        return false;
    }
    if let Some(old_generation) = old_generation {
        if !try_link_down(tx, link, old_generation) {
            return false;
        }
    }
    tx.try_control(TalEvent::LinkUp { link, initiator: true, quality }, new_generation)
        .is_ok()
}

async fn remove_if_current(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &EventSender,
    link: LinkId,
    generation: u64,
    write_gate: &Arc<AsyncMutex<()>>,
) -> bool {
    let _event_guard = event_gate.lock().await;
    let _write_guard = acquire_write_gate(write_gate).await;
    let mut table = links.lock().await;
    if table.get(&link).map(|state| state.generation) != Some(generation) {
        return false;
    }
    if table.remove(&link).is_none() {
        return false;
    }
    drop(table);
    try_link_down(tx, link, generation)
}

/// Enqueue a lifecycle close without awaiting the public receiver.  A full
/// control ledger is a fail-closed overload: callers have already removed the
/// link from the live table, and the ingress has no stale data for it.
fn try_link_down(tx: &EventSender, link: LinkId, generation: u64) -> bool {
    match tx.try_control(TalEvent::LinkDown { link }, generation) {
        Ok(()) => true,
        Err(ControlEnqueueError::Full | ControlEnqueueError::InvalidEvent) => false,
    }
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
    tx: EventSender,
    // `std::sync::Mutex` (not tokio's) is correct here: it is only ever held
    // for the instant it takes to `Option::take`, never across an `.await`.
    events_rx: std::sync::Mutex<Option<mpsc::Receiver<TalEvent>>>,
    links: Arc<AsyncMutex<HashMap<LinkId, LinkState>>>,
    event_gate: Arc<AsyncMutex<()>>,
    /// Serializes dials so a reconnect cannot race the two-phase replacement
    /// of the same deterministic LinkId.
    dial_gate: Arc<AsyncMutex<()>>,
    next_generation: AtomicU64,
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
        let event_gate = Arc::new(AsyncMutex::new(()));
        let dial_gate = Arc::new(AsyncMutex::new(()));

        tokio::spawn(watch_adapter(
            adapter.clone(),
            tx.clone(),
            links.clone(),
            event_gate.clone(),
        ));

        Ok(BleCentral {
            adapter,
            tx,
            events_rx: std::sync::Mutex::new(Some(rx)),
            links,
            event_gate,
            dial_gate,
            next_generation: AtomicU64::new(0),
        })
    }

    /// Take ownership of this central's event stream. There is exactly one
    /// receiver per `BleCentral`; calling this a second time panics — hold
    /// on to the first receiver instead of calling this again. The receiver
    /// must be drained while the central is active. Link-data overflow retires
    /// the affected link; beacon overflow is intentionally lossy.
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
        let _dial_guard = self.dial_gate.lock().await;
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

        let (previous_state, old_generation, previous_guard) = {
            let _event_guard = self.event_gate.lock().await;
            let previous = self
                .links
                .lock()
                .await
                .get(&link)
                .map(|state| (state.generation, state.write_gate.clone()));
            let previous_guard = match previous.as_ref() {
                Some((_, gate)) => Some(acquire_write_gate(gate).await),
                None => None,
            };
            let previous_state = {
                let mut links = self.links.lock().await;
                match previous.as_ref() {
                    Some((generation, _))
                        if links.get(&link).map(|state| state.generation) == Some(*generation) =>
                    {
                        links.remove(&link)
                    }
                    _ => None,
                }
            };
            let old_generation = previous.as_ref().and_then(|(generation, _)| {
                previous_state.as_ref().map(|_| *generation)
            });
            if let Some(generation) = old_generation {
                // Retire the old ingress generation before the potentially
                // slow external disconnect. The replacement task repeats the
                // coalescing Down defensively, preserving Down→Up ordering.
                let _ = try_link_down(&self.tx, link, generation);
            }
            (previous_state, old_generation, previous_guard)
        };
        // The write guard has already waited for any in-flight encrypted ATT
        // sequence, and the old link was removed from the table while the
        // lifecycle gate was held.  Release it before the bounded external
        // disconnect so no local task is held behind controller latency.
        drop(previous_guard);
        if let Some(previous_state) = previous_state {
            disconnect_bounded(&previous_state.peripheral).await;
        }
        let generation = allocate_generation(&self.next_generation);
        let write_gate = Arc::new(AsyncMutex::new(()));
        {
            let _event_guard = self.event_gate.lock().await;
            self.links.lock().await.insert(
                link,
                LinkState {
                    peripheral,
                    tx_characteristic,
                    chunk_size: DEFAULT_CHUNK,
                    write_gate: write_gate.clone(),
                    generation,
                },
            );
        }

        let quality = LinkQuality { mtu: (DEFAULT_CHUNK + 3) as u16, ..LinkQuality::default() };
        let data_tx = self.tx.clone();
        let rx_uuid = rx_characteristic.uuid;
        let links = self.links.clone();
        let event_gate = self.event_gate.clone();
        tokio::spawn(async move {
            if !emit_replacement(
                &links,
                &event_gate,
                &data_tx,
                link,
                old_generation,
                generation,
                quality,
            )
            .await
            {
                // Control events have reserved capacity, but a saturated
                // ledger is still a fail-closed condition.  Retire the
                // installed link so no live state remains without a
                // corresponding lifecycle event.
                let _ = remove_if_current(
                    &links,
                    &event_gate,
                    &data_tx,
                    link,
                    generation,
                    &write_gate,
                )
                .await;
                return;
            }
            while let Some(notification) = notifications.next().await {
                if notification.uuid == rx_uuid
                    && !emit_if_current(
                        &links,
                        &event_gate,
                        &data_tx,
                        link,
                        generation,
                        TalEvent::LinkData { link, bytes: notification.value },
                    )
                    .await
                {
                    break;
                }
            }
            let _ = remove_if_current(
                &links,
                &event_gate,
                &data_tx,
                link,
                generation,
                &write_gate,
            )
            .await;
        });

        Ok(link)
    }

    /// Chunk `bytes` into Write-Without-Response ATT payloads and send them,
    /// in order, on `link`'s TX characteristic.
    pub async fn send(&self, link: LinkId, bytes: &[u8]) -> Result<(), BleError> {
        let (peripheral, tx_characteristic, chunk_size, write_gate, generation) = {
            let links = self.links.lock().await;
            let state = links.get(&link).ok_or(BleError::UnknownLink)?;
            (
                state.peripheral.clone(),
                state.tx_characteristic.clone(),
                state.chunk_size,
                state.write_gate.clone(),
                state.generation,
            )
        };

        let _write_guard = acquire_write_gate(&write_gate).await;
        let current_generation = self
            .links
            .lock()
            .await
            .get(&link)
            .map(|state| state.generation);
        if !generation_is_current(current_generation, generation) {
            return Err(BleError::UnknownLink);
        }
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
        let (state, write_guard) = {
            let _event_guard = self.event_gate.lock().await;
            let previous = self
                .links
                .lock()
                .await
                .get(&link)
                .map(|state| (state.generation, state.write_gate.clone()));
            let Some((generation, gate)) = previous else {
                let _ = try_link_down(&self.tx, link, 0);
                return;
            };
            let write_guard = acquire_write_gate(&gate).await;
            let state = {
                let mut links = self.links.lock().await;
                if links.get(&link).map(|state| state.generation) == Some(generation) {
                    links.remove(&link)
                } else {
                    None
                }
            };
            if state.is_some() {
                let _ = try_link_down(&self.tx, link, generation);
            }
            (state, Some(write_guard))
        };
        // State removal plus the acquired guard makes the old write sequence
        // quiescent.  Do not retain this local guard across the controller
        // await below.
        drop(write_guard);
        if let Some(state) = state {
            disconnect_bounded(&state.peripheral).await;
        }
    }

    /// Retire and physically disconnect every live link. Hosts should call
    /// this on shutdown and after observing [`Self::control_events_dropped`]
    /// so a saturated lifecycle queue cannot leave a radio link alive.
    pub async fn shutdown_all(&self) {
        shutdown_links(&self.links, &self.event_gate, &self.tx).await;
    }

    /// Return the sticky count of lifecycle events rejected because the
    /// transport's bounded control lanes were saturated. A non-zero value is
    /// a fail-closed operational signal; call [`Self::shutdown_all`] before
    /// attempting further admission.
    pub fn control_events_dropped(&self) -> u64 {
        self.tx.control_events_dropped()
    }
}

/// Background task: turns the adapter's `CentralEvent` stream into
/// `TalEvent::Beacon` (on Service Data ads matching our UUID) and
/// `TalEvent::LinkDown` (on disconnects of links we still track). Exits when
/// the adapter's event stream ends or nobody is listening on `tx` anymore.
async fn watch_adapter(
    adapter: Adapter,
    tx: EventSender,
    links: Links,
    event_gate: Arc<AsyncMutex<()>>,
) {
    let uuid = service_uuid();
    let mut events = match adapter.events().await {
        Ok(stream) => stream,
        Err(_) => {
            shutdown_links(&links, &event_gate, &tx).await;
            return;
        }
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
                let _ = tx.try_data(TalEvent::Beacon { payload: payload.clone(), rssi });
            }
            CentralEvent::DeviceDisconnected(id) => {
                let link = mint_link_id(&id);
                let previous = links
                    .lock()
                    .await
                    .get(&link)
                    .map(|state| {
                        (
                            state.generation,
                            state.write_gate.clone(),
                            state.peripheral.clone(),
                        )
                    });
                let Some((generation, write_gate, peripheral)) = previous else {
                    continue;
                };
                if is_connected_bounded(&peripheral).await {
                    continue;
                }
                let _event_guard = event_gate.lock().await;
                let _write_guard = acquire_write_gate(&write_gate).await;
                let removed = {
                    let mut table = links.lock().await;
                    if table.get(&link).map(|state| state.generation) == Some(generation) {
                        table.remove(&link).is_some()
                    } else {
                        false
                    }
                };
                if removed {
                    let _ = try_link_down(&tx, link, generation);
                }
            }
            _ => {}
        }
    }
    shutdown_links(&links, &event_gate, &tx).await;
}

/// Retire all central links when the adapter event stream ends. The lifecycle
/// gate and per-link write gates are used only for in-process state; actual
/// BlueZ disconnect calls happen after the gate is released and are bounded so
/// an unresponsive adapter cannot strand the rest of the transport.
async fn shutdown_links(links: &Links, event_gate: &Arc<AsyncMutex<()>>, tx: &EventSender) {
    let peripherals = {
        let _event_guard = event_gate.lock().await;
        let snapshots = {
            let table = links.lock().await;
            table
                .iter()
                .map(|(&link, state)| {
                    (link, state.generation, state.write_gate.clone(), state.peripheral.clone())
                })
                .collect::<Vec<_>>()
        };
        let mut peripherals = Vec::new();
        for (link, generation, write_gate, peripheral) in snapshots {
            let _write_guard = acquire_write_gate(&write_gate).await;
            let removed = {
                let mut table = links.lock().await;
                if table.get(&link).map(|state| state.generation) == Some(generation) {
                    table.remove(&link).is_some()
                } else {
                    false
                }
            };
            if removed {
                let _ = try_link_down(tx, link, generation);
                peripherals.push(peripheral);
            }
        }
        peripherals
    };
    for peripheral in peripherals {
        disconnect_bounded(&peripheral).await;
    }
}

async fn disconnect_bounded(peripheral: &Peripheral) {
    let _ = timeout(Duration::from_secs(2), peripheral.disconnect()).await;
}

async fn is_connected_bounded(peripheral: &Peripheral) -> bool {
    timeout(Duration::from_secs(1), peripheral.is_connected())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false)
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
    use super::{
        acquire_write_gate, allocate_generation, generation_is_current, mint_link_id,
    };
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use tokio::sync::Mutex as AsyncMutex;

    #[test]
    fn generation_guard_rejects_stale_and_absent_links() {
        assert!(generation_is_current(Some(7), 7));
        assert!(!generation_is_current(Some(8), 7));
        assert!(!generation_is_current(None, 7));
    }

    #[test]
    fn generations_are_monotonic_across_reconnects() {
        let counter = AtomicU64::new(0);
        let first = allocate_generation(&counter);
        let second = allocate_generation(&counter);
        assert_eq!((first, second), (1, 2));

        // A stale callback from generation one cannot match the replacement;
        // the replacement remains the only current generation.
        assert!(!generation_is_current(Some(second), first));
        assert!(generation_is_current(Some(second), second));
    }

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

    #[test]
    fn mint_link_id_is_deterministic_for_the_same_peer() {
        assert_eq!(mint_link_id(&"peer-a"), mint_link_id(&"peer-a"));
    }

    #[test]
    fn mint_link_id_differs_across_distinct_peers() {
        assert_ne!(mint_link_id(&"peer-a"), mint_link_id(&"peer-b"));
    }
}
