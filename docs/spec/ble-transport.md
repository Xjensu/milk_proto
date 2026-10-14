# BLE transport

Status: normative MMP/1 source

The discovery record is exactly 24 bytes:

```text
byte 0: version:4, tier:2, battery:2
bytes 1..8: ephemeral identifier
byte 9: busy:1, epoch:7
bytes 10..11: fragment structure:2, fragment offset:14
bytes 12..23: summary fragment
```

Beacon identifiers rotate every 15 minutes in the reference policy. A receiver MUST treat a beacon as a hint and MUST rate-limit scan-triggered work. It MUST NOT derive an identity, store a bundle, or grant a copy budget from beacon content alone.

The bulk plane uses one MMP service and TX/RX characteristics. The reference development UUIDs are:

```text
service 6d6d7031-0000-4d4d-5031-6d6573686d6c
TX      6d6d7031-0001-4d4d-5031-6d6573686d6c
RX      6d6d7031-0002-4d4d-5031-6d6573686d6c
```

They are development identifiers. A production profile MUST select and document a stable UUID allocation before release. Application data is fragmented at the negotiated ATT MTU minus the ATT header. The btleplug fallback chunk is 20 bytes when the effective MTU is unknown.

The transport abstraction reports link-up, data, write failure, timeout, and link-down events. A failed write or liveness check retires the link and clears its pending data. Session code MUST bound queue growth and MUST NOT wait indefinitely for a platform callback.

The Rust central and Linux peripheral paths exist. Android `BluetoothGattServer`, iOS CoreBluetooth, mobile background policy, and physical cross-platform behavior remain unverified. Bluetooth Channel Sounding and controller filtering are optional research inputs; no MMP/1 requirement assumes their presence.
