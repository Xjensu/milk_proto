//! Linux/BlueZ backend via `bluer` (the official BlueZ D-Bus bindings).
//!
//! Uses bluer's *IO* GATT-server methods (`AcquireWrite`/`AcquireNotify`
//! under the hood): BlueZ hands us a packet socket per direction, each
//! datagram is exactly one ATT write / one notification, and — crucially —
//! each socket reports the **exchanged ATT MTU** (`CharacteristicReader::mtu`
//! / `CharacteristicWriter::mtu`), which is the negotiated-MTU signal the
//! central crate never gets from btleplug (module docs, decision 6).
//!
//! ## Role inversion, concretely (decision 4)
//!
//! | UUID       | central crate's name & act | this backend                        |
//! |------------|----------------------------|-------------------------------------|
//! | `…-0001-…` | "TX" — central WRITES      | Write-Without-Response → `LinkData` |
//! | `…-0002-…` | "RX" — central SUBSCRIBES  | Notify ← [`PeripheralRole::notify`] |
//!
//! ## Link lifecycle
//!
//! One connected central = one link (handbook §4.1). A link comes up on the
//! FIRST of (central subscribes to notify, central's first write arrives) —
//! in practice subscribe comes first because `BleCentral::dial` subscribes
//! before it emits its own `LinkUp`, and the MMP session has the dialer speak
//! first (HELLO, doc 11) — and `LinkUp { initiator: false }` is emitted once.
//! `LinkDown` is emitted when the current write/read socket closes (BlueZ
//! closes both sockets on disconnect) or when a `notify` send fails. Each
//! callback carries a direction-specific endpoint token; only the current
//! token may remove the link, so delayed callbacks from a stable-ID
//! reconnect are rejected before they can emit data or teardown.
//! Known, accepted gap: a central that subscribes but never writes is only
//! detected as gone on the next failed `notify` — the session layer's idle
//! timers (doc 11 §7) sit above us and bound that window anyway.

use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use bluer::adv::{Advertisement, AdvertisementHandle};
use bluer::gatt::local::{
    characteristic_control, Application, ApplicationHandle, Characteristic,
    CharacteristicControlEvent, CharacteristicNotify, CharacteristicNotifyMethod,
    CharacteristicWrite, CharacteristicWriteMethod, Service,
};
use bluer::gatt::{CharacteristicReader, CharacteristicWriter};
use bluer::{Adapter, Session, Uuid};
use futures::{pin_mut, StreamExt};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::framing;
use crate::{
    event_channel, mint_link_id, LinkId, LinkQuality, PeriphError, TalEvent,
    DEV_CENTRAL_RX_CHARACTERISTIC_UUID,
    DEV_CENTRAL_TX_CHARACTERISTIC_UUID, DEV_SERVICE_UUID,
};
use crate::lifecycle::{AttachResult, LinkLifecycle, StreamSide, StreamToken};

/// Per-link state between `LinkUp` and `LinkDown`. Reader and writer endpoint
/// tokens are kept alongside the writer so stale callbacks can be rejected;
/// the write gate serializes notifications with same-direction replacement.
struct LinkState {
    /// Outbound notify socket; `None` until the central subscribes. Wrapped
    /// in its own mutex (not just the table's) so a long `notify` on one link
    /// never holds the whole link table.
    writer: Option<Arc<AsyncMutex<CharacteristicWriter>>>,
    /// `max(negotiated ATT MTU − 3, 20)` — see `framing::chunk_size_for_mtu`.
    chunk_size: usize,
    reader_token: Option<StreamToken>,
    writer_token: Option<StreamToken>,
    write_gate: Arc<AsyncMutex<()>>,
}

struct LinkRegistry {
    lifecycle: LinkLifecycle,
    states: HashMap<LinkId, LinkState>,
}

type Links = Arc<AsyncMutex<LinkRegistry>>;

/// A live BLE peripheral: one BlueZ adapter, a registered GATT application,
/// at most one advertisement, and a table of links centrals dialed into us.
///
/// Owns no runtime — construct and drive it from the host's tokio runtime.
/// Dropping it unregisters the GATT application and the advertisement.
pub struct PeripheralRole {
    adapter: Adapter,
    // Handles are kept purely for their Drop side (D-Bus unregistration).
    _session: Session,
    _app: ApplicationHandle,
    adv: AsyncMutex<Option<AdvertisementHandle>>,
    tx: mpsc::Sender<TalEvent>,
    // `std::sync::Mutex` (not tokio's) is correct: held only for the instant
    // of an `Option::take`, never across an `.await`.
    events_rx: std::sync::Mutex<Option<mpsc::Receiver<TalEvent>>>,
    links: Links,
    event_gate: Arc<AsyncMutex<()>>,
}

impl PeripheralRole {
    /// Connect to the BlueZ daemon, power the default adapter, and register
    /// the MMP GATT application (service + both characteristics). Must be
    /// called on a tokio runtime. Advertising starts only on [`Self::advertise`].
    pub async fn new() -> Result<Self, PeriphError> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        // Two independent control streams: one per characteristic, so a
        // write-request event can never be confused with a notify-accept.
        let (write_ctl, write_handle) = characteristic_control();
        let (notify_ctl, notify_handle) = characteristic_control();

        let app = Application {
            services: vec![Service {
                uuid: service_uuid(),
                primary: true,
                characteristics: vec![
                    // ROLE INVERSION (decision 4): the central's "TX" is our
                    // inbound write-without-response characteristic.
                    Characteristic {
                        uuid: central_tx_uuid(),
                        write: Some(CharacteristicWrite {
                            write_without_response: true,
                            method: CharacteristicWriteMethod::Io,
                            ..Default::default()
                        }),
                        control_handle: write_handle,
                        ..Default::default()
                    },
                    // …and the central's "RX" is our outbound notify one.
                    Characteristic {
                        uuid: central_rx_uuid(),
                        notify: Some(CharacteristicNotify {
                            notify: true,
                            method: CharacteristicNotifyMethod::Io,
                            ..Default::default()
                        }),
                        control_handle: notify_handle,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let app_handle = adapter.serve_gatt_application(app).await?;

        let (tx, rx) = event_channel();
        let links: Links = Arc::new(AsyncMutex::new(LinkRegistry {
            lifecycle: LinkLifecycle::default(),
            states: HashMap::new(),
        }));
        let event_gate = Arc::new(AsyncMutex::new(()));

        tokio::spawn(run_control(
            write_ctl,
            notify_ctl,
            tx.clone(),
            links.clone(),
            event_gate.clone(),
        ));

        Ok(PeripheralRole {
            adapter,
            _session: session,
            _app: app_handle,
            adv: AsyncMutex::new(None),
            tx,
            events_rx: std::sync::Mutex::new(Some(rx)),
            links,
            event_gate,
        })
    }

    /// (Re-)start advertising with `beacon` as MMP service data. The payload
    /// is opaque here — `Node::beacon()` mints it; the host re-calls this
    /// whenever [`crate::readvertise_due`] fires (15-min `EPHEM_SLOT_MS`
    /// rotation, doc 09 §3.2).
    ///
    /// BlueZ has no "mutate a live advertisement" call, so each call
    /// unregisters the previous advertisement *first*, then registers the new
    /// one — ordering chosen so we never hold two of the controller's scarce
    /// advertising instances; the millisecond gap per 15 min is harmless.
    ///
    /// Byte-budget caveat (dev placeholder, OQ-12): a 128-bit service-data
    /// UUID + 24-byte payload cannot fit the 31-byte legacy PDU that doc 09
    /// §3.1 lays out for the eventual 16-bit SIG UUID; BlueZ will use
    /// extended advertising or scan-response placement where the controller
    /// allows. This matches what the central crate scans for today
    /// (service data keyed by the same 128-bit UUID), which is the contract
    /// that matters for M5b.
    pub async fn advertise(&self, beacon: Vec<u8>) -> Result<(), PeriphError> {
        let adv = Advertisement {
            advertisement_type: bluer::adv::Type::Peripheral, // connectable
            service_uuids: [service_uuid()].into_iter().collect(),
            service_data: [(service_uuid(), beacon)].into_iter().collect(),
            discoverable: Some(true),
            ..Default::default()
        };
        let mut slot = self.adv.lock().await;
        *slot = None; // drop (unregister) the previous advertisement first
        *slot = Some(self.adapter.advertise(adv).await?);
        Ok(())
    }

    /// Take ownership of this peripheral's event stream
    /// (`LinkUp { initiator: false }` / `LinkData` / `LinkDown`). There is
    /// exactly one receiver per `PeripheralRole`; calling this a second time
    /// panics — hold on to the first receiver instead. (Same contract as
    /// `BleCentral::events`.) Drain it while the peripheral is active: the
    /// bounded queue applies backpressure rather than dropping ATT writes.
    pub fn events(&self) -> mpsc::Receiver<TalEvent> {
        self.events_rx
            .lock()
            .expect("events_rx mutex poisoned")
            .take()
            .expect(
                "PeripheralRole::events() called more than once; only one receiver is handed out",
            )
    }

    /// Chunk `bytes` to the link's negotiated size and push them, in order,
    /// as GATT notifications. Errors with [`PeriphError::NotSubscribed`] if
    /// the central hasn't subscribed yet; a mid-stream send failure tears the
    /// link down (emits `LinkDown`) and surfaces the BlueZ error.
    pub async fn notify(&self, link: LinkId, bytes: &[u8]) -> Result<(), PeriphError> {
        let (writer, chunk_size, token, write_gate) = {
            let links = self.links.lock().await;
            let state = links.states.get(&link).ok_or(PeriphError::UnknownLink)?;
            (
                state.writer.clone().ok_or(PeriphError::NotSubscribed)?,
                state.chunk_size,
                state.writer_token.ok_or(PeriphError::NotSubscribed)?,
                state.write_gate.clone(),
            )
        };

        let _write_guard = write_gate.lock_owned().await;
        if !self.links.lock().await.lifecycle.is_current(token) {
            return Err(PeriphError::UnknownLink);
        }
        let writer = writer.lock().await;
        for chunk in framing::chunks(bytes, chunk_size) {
            if !self.links.lock().await.lifecycle.is_current(token) {
                return Err(PeriphError::UnknownLink);
            }
            if let Err(e) = writer.send(chunk).await {
                drop(writer);
                drop(_write_guard);
                link_down_if_current(&self.links, &self.event_gate, &self.tx, token).await;
                // send() speaks io::Error (it is a socket write); bluer's
                // Error has a lossless From impl for it.
                return Err(PeriphError::Bluer(e.into()));
            }
        }
        Ok(())
    }
}

/// Background task owning both characteristic-control streams. Ends when the
/// GATT application is unregistered (both streams close) or when nobody is
/// listening for events anymore.
async fn run_control(
    write_ctl: bluer::gatt::local::CharacteristicControl,
    notify_ctl: bluer::gatt::local::CharacteristicControl,
    tx: mpsc::Sender<TalEvent>,
    links: Links,
    event_gate: Arc<AsyncMutex<()>>,
) {
    pin_mut!(write_ctl);
    pin_mut!(notify_ctl);
    loop {
        tokio::select! {
            evt = write_ctl.next() => match evt {
                // Central started writing into the write-without-response
                // characteristic: accept the packet socket and shuttle every
                // datagram (= one ATT write) out as `LinkData`.
                Some(CharacteristicControlEvent::Write(req)) => {
                    let Ok(reader) = req.accept() else { continue };
                    let link = mint_link_id(&reader.device_address());
                    let endpoint = reader.as_raw_fd() as i64;
                    let Some(attach) = ensure_link_up(
                        &links,
                        &event_gate,
                        &tx,
                        link,
                        reader.mtu(),
                        StreamSide::Reader,
                        endpoint,
                        None,
                    )
                    .await else {
                        continue;
                    };
                    tokio::spawn(read_loop(
                        reader,
                        attach.token,
                        tx.clone(),
                        links.clone(),
                        event_gate.clone(),
                    ));
                }
                Some(_) => {}
                None => return,
            },
            evt = notify_ctl.next() => match evt {
                // Central subscribed to the notify characteristic: this is
                // the mirror moment of `BleCentral::dial`'s own LinkUp.
                Some(CharacteristicControlEvent::Notify(writer)) => {
                    let link = mint_link_id(&writer.device_address());
                    let mtu = writer.mtu();
                    let endpoint = writer.as_raw_fd() as i64;
                    let _ = ensure_link_up(
                        &links,
                        &event_gate,
                        &tx,
                        link,
                        mtu,
                        StreamSide::Writer,
                        endpoint,
                        Some(writer),
                    )
                    .await;
                }
                Some(_) => {}
                None => return,
            },
        }
    }
}

/// Insert-or-update one endpoint. The first endpoint emits `LinkUp`; a
/// same-direction endpoint replacement emits `LinkDown` then `LinkUp` under
/// the event gate. Both directions negotiate one ATT MTU, so whichever
/// endpoint reports first sets the chunk size; a later report for the same
/// link refreshes it (idempotent in practice — same value).
async fn ensure_link_up(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &mpsc::Sender<TalEvent>,
    link: LinkId,
    att_mtu: usize,
    side: StreamSide,
    endpoint: i64,
    writer: Option<CharacteristicWriter>,
) -> Option<AttachResult> {
    let chunk_size = framing::chunk_size_for_mtu(att_mtu);
    let _event_guard = event_gate.lock().await;
    let old_write_gate = links
        .lock()
        .await
        .states
        .get(&link)
        .map(|state| state.write_gate.clone());
    let _old_write_guard = match old_write_gate {
        Some(gate) => Some(gate.lock_owned().await),
        None => None,
    };

    let mut links = links.lock().await;
    let attach = links.lifecycle.attach(link, side, endpoint)?;
    let state = if attach.restarted {
        links.states.insert(
            link,
            LinkState {
                writer: None,
                chunk_size,
                reader_token: None,
                writer_token: None,
                write_gate: Arc::new(AsyncMutex::new(())),
            },
        );
        links.states.get_mut(&link).expect("inserted link state")
    } else {
        links.states.entry(link).or_insert_with(|| LinkState {
            writer: None,
            chunk_size,
            reader_token: None,
            writer_token: None,
            write_gate: Arc::new(AsyncMutex::new(())),
        })
    };
    state.chunk_size = chunk_size;
    match side {
        StreamSide::Reader => state.reader_token = Some(attach.token),
        StreamSide::Writer => {
            let replace_writer = state.writer_token != Some(attach.token);
            state.writer_token = Some(attach.token);
            if replace_writer {
                if let Some(w) = writer {
                    state.writer = Some(Arc::new(AsyncMutex::new(w)));
                }
            }
        }
    }

    drop(links);
    if attach.restarted {
        if tx.send(TalEvent::LinkDown { link }).await.is_err() {
            return Some(attach);
        }
    }
    if attach.link_was_new || attach.restarted {
        let quality =
            LinkQuality { mtu: (chunk_size + framing::ATT_HEADER) as u16, ..LinkQuality::default() };
        let _ = tx.send(TalEvent::LinkUp { link, initiator: false, quality }).await;
    }
    Some(attach)
}

/// Forward every inbound ATT write as `LinkData`; when the socket closes
/// (central released it or disconnected — BlueZ closes it either way), tear
/// the link down.
async fn read_loop(
    reader: CharacteristicReader,
    token: StreamToken,
    tx: mpsc::Sender<TalEvent>,
    links: Links,
    event_gate: Arc<AsyncMutex<()>>,
) {
    loop {
        match reader.recv().await {
            // A 0-byte datagram from a packet socket is indistinguishable
            // from EOF; MMP chunks are never empty (framing::chunks yields no
            // empty slices), so treat it as end-of-stream.
            Ok(bytes) if bytes.is_empty() => break,
            Ok(bytes) => {
                if !emit_data_if_current(&links, &event_gate, &tx, token, bytes).await {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    link_down_if_current(&links, &event_gate, &tx, token).await;
}

async fn emit_data_if_current(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &mpsc::Sender<TalEvent>,
    token: StreamToken,
    bytes: Vec<u8>,
) -> bool {
    let _event_guard = event_gate.lock().await;
    let is_current = links.lock().await.lifecycle.is_current(token);
    if !is_current {
        return false;
    }
    tx.send(TalEvent::LinkData { link: token.link, bytes }).await.is_ok()
}

/// Remove-then-emit under the lifecycle gate: only the callback that owns the
/// current endpoint token can retire the logical link or emit `LinkDown`.
async fn link_down_if_current(
    links: &Links,
    event_gate: &Arc<AsyncMutex<()>>,
    tx: &mpsc::Sender<TalEvent>,
    token: StreamToken,
) {
    let _event_guard = event_gate.lock().await;
    let write_gate = links
        .lock()
        .await
        .states
        .get(&token.link)
        .map(|state| state.write_gate.clone());
    let Some(write_gate) = write_gate else { return };
    let _write_guard = write_gate.lock_owned().await;
    let mut links = links.lock().await;
    if !links.lifecycle.close_if_current(token) {
        return;
    }
    links.states.remove(&token.link);
    drop(links);
    let _ = tx.send(TalEvent::LinkDown { link: token.link }).await;
}

fn service_uuid() -> Uuid {
    DEV_SERVICE_UUID.parse().expect("DEV_SERVICE_UUID is a valid UUID literal")
}

fn central_tx_uuid() -> Uuid {
    DEV_CENTRAL_TX_CHARACTERISTIC_UUID
        .parse()
        .expect("DEV_CENTRAL_TX_CHARACTERISTIC_UUID is a valid UUID literal")
}

fn central_rx_uuid() -> Uuid {
    DEV_CENTRAL_RX_CHARACTERISTIC_UUID
        .parse()
        .expect("DEV_CENTRAL_RX_CHARACTERISTIC_UUID is a valid UUID literal")
}
