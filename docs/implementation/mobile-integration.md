# Mobile and desktop integration

`mmp-ffi` exposes bounded polling, link lifecycle, event delivery, and snapshot seams to Kotlin, Swift, and React Native consumers. The Rust mobile transport fragments writes at the effective ATT MTU and uses defensive copies at the ABI boundary.

The Android and iOS coordinator code can be compiled without platform SDKs, but this checkout has no Android NDK, Xcode SDK, or native app shells. Android `BluetoothGattServer`, iOS CoreBluetooth central/peripheral behavior, foreground/background policy, keystore/keychain persistence, reconnect, Doze, and restoration remain external gates. A native consumer compile and a two-device physical test are required before making a mobile interoperability claim.

Linux uses the btleplug central and a BlueZ peripheral path. Windows and macOS peripheral backends are stubs in the current workspace. Development UUIDs are not production allocations.
