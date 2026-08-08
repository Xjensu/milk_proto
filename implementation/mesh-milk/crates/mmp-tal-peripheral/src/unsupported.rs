//! Stub backend for OSes without a peripheral implementation yet (decision 2:
//! Linux/BlueZ shipped first; Windows and macOS are deliberate,
//! clearly-marked stubs rather than half-working attempts).
//!
//! ## Windows design sketch (what a real backend would take)
//!
//! WinRT via the `windows` crate — features `Devices_Bluetooth`,
//! `Devices_Bluetooth_GenericAttributeProfile`, `Devices_Bluetooth_Advertisement`,
//! `Storage_Streams`, `Foundation_Collections`:
//!
//! - **GATT server:** `GattServiceProvider::CreateAsync(DEV_SERVICE_UUID)`;
//!   two `GattLocalCharacteristic`s created with
//!   `GattLocalCharacteristicParameters` — `…-0001-…` with
//!   `GattCharacteristicProperties::WriteWithoutResponse` (hook
//!   `WriteRequested` → `TalEvent::LinkData`), `…-0002-…` with
//!   `::Notify` (outbound via `NotifyValueAsync`). Per-subscriber
//!   `GattSubscribedClient::MaxNotificationSize` is a real per-link MTU
//!   signal, analogous to the BlueZ one this crate already exploits;
//!   `SubscribedClientsChanged` maps to `LinkUp`/`LinkDown` (mint the
//!   `LinkId` from the client's `Session.DeviceId` Debug string, same
//!   recipe). `GattSession::MaxPduSize` covers the write side.
//! - **Advertising the beacon:** the hard part. The legacy
//!   `BluetoothLEAdvertisementPublisher` historically rejects most AD types
//!   (service data included) unless the Windows 11 extended-advertising
//!   properties (`UseExtendedAdvertisement`) are available and the radio
//!   supports them; the fallback is
//!   `GattServiceProviderAdvertisingParameters` (`IsConnectable` +
//!   `IsDiscoverable`, plus the `ServiceData` property present on newer
//!   builds) on `StartAdvertising`. Which combination actually carries the
//!   24-byte service-data beacon varies by build and radio — it needs
//!   empirical validation on target hardware, which is exactly why this stub
//!   exists instead of unverifiable code.
//!
//! ## macOS
//!
//! Possible (CoreBluetooth `CBPeripheralManager` via `objc2` bindings) but
//! explicitly lowest priority (handbook §4.1); CoreBluetooth also rewrites
//! advertising payloads (no raw service-data control), which needs its own
//! design pass.
//!
//! The stub keeps the exact public API so cross-platform hosts (`mmp-cli
//! listen`) compile everywhere and fail at runtime with a self-explanatory
//! [`PeriphError::Unsupported`].

use tokio::sync::mpsc;

use crate::{LinkId, PeriphError, TalEvent};

#[cfg(target_os = "windows")]
const PLATFORM_NOTE: &str =
    "Windows peripheral backend not implemented yet (WinRT GattServiceProvider; \
     see mmp-tal-peripheral/src/unsupported.rs for the design sketch)";
#[cfg(target_os = "macos")]
const PLATFORM_NOTE: &str =
    "macOS peripheral backend not implemented (CBPeripheralManager; lowest priority per handbook)";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PLATFORM_NOTE: &str = "no BLE peripheral backend for this OS";

/// Uninhabitable helper: proves to the compiler (and the reader) that no
/// `PeripheralRole` value can exist on this OS — `new()` always errors, so
/// the other methods are statically unreachable rather than `unimplemented!`.
enum Never {}

/// Stub with the same API as the Linux backend; see module docs.
pub struct PeripheralRole {
    never: Never,
}

impl PeripheralRole {
    /// Always fails on this OS; the error text says why and where the design
    /// sketch lives.
    pub async fn new() -> Result<Self, PeriphError> {
        Err(PeriphError::Unsupported(PLATFORM_NOTE))
    }

    /// Statically unreachable (no value of `Self` can exist).
    pub async fn advertise(&self, _beacon: Vec<u8>) -> Result<(), PeriphError> {
        match self.never {}
    }

    /// Statically unreachable (no value of `Self` can exist).
    pub fn events(&self) -> mpsc::Receiver<TalEvent> {
        match self.never {}
    }

    /// Statically unreachable (no value of `Self` can exist).
    pub async fn notify(&self, _link: LinkId, _bytes: &[u8]) -> Result<(), PeriphError> {
        match self.never {}
    }
}
