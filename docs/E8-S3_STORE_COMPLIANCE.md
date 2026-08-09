# E8-S3 Store-Compliance Dossier

Status: engineering guidance and release evidence requirements; not legal
advice and not a promise of Google Play or App Store approval.

Access date for every external source below: **2026-08-09**.

This dossier closes the documentation part of E8-S3. It does not claim that
`mmp-ffi` is a compliant mobile application: the repository currently ships a
Rust library, generated bindings, and structural artifact checks, but no
Android or iOS consuming application with a manifest, capabilities, privacy
copy, foreground-service implementation, or CoreBluetooth lifecycle tests.

## 1. Current repository boundary

`implementation/mesh-milk/MOBILE_INTEGRATION.md` and
`implementation/mesh-milk/REACT_NATIVE_INTEGRATION.md` correctly assign the
radio, wakeups, and secret storage to the platform shell. Their service and
state-restoration examples are integration guidance, not evidence that the
platform behavior has been implemented or accepted by a store.

The current release status is therefore:

| Area | Evidence currently available | What is still blocked |
| --- | --- | --- |
| Rust/FFI API | Generated API and secret-surface checks; AAR/XCFramework shape validators | Native app lifecycle and permission behavior |
| Android | Design for a foreground BLE service and monotonic `tick()` calls | Manifest/service type, runtime grants, Doze/OEM field tests, Play Console declaration |
| iOS | CoreBluetooth integration seam and restoration identifiers in examples | Entitlements/capabilities, iOS 26 relaunch rules, physical-device restoration tests, App Review submission |

## 2. Android: foreground service, BLE permissions, and Doze

### Platform facts (official documentation)

1. For apps targeting Android 14/API 34 or newer, every foreground service
   must declare a service type and, where applicable, its corresponding
   foreground-service permission. Android documents `connectedDevice` for
   interactions with external devices that require Bluetooth and lists
   `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`, and `BLUETOOTH_ADVERTISE` as runtime
   prerequisites for that type.
2. For Android 12/API 31 or newer, those Bluetooth permissions are runtime
   permissions. `BLUETOOTH_SCAN` is for discovery, `BLUETOOTH_ADVERTISE` for
   making the device discoverable, and `BLUETOOTH_CONNECT` for communicating
   with devices. `neverForLocation` is only appropriate when the app can
   strongly assert that scan results are never used to derive physical
   location; Android warns that this flag can filter some beacons.
3. Doze suspends network access, ignores wake locks, and defers ordinary
   alarms. `setAndAllowWhileIdle()`/`setExactAndAllowWhileIdle()` are the
   documented exceptions, but Android limits either form to no more than one
   firing per nine minutes per app. A foreground service prevents App Standby
   classification while it is running, but it is not a promise of unrestricted
   radio, CPU, or OEM behavior.
4. Google Play requires an Android 14+ foreground-service declaration in Play
   Console, including the use case, user impact if deferred/interrupted, and a
   demonstration video. The policy describes foreground services as user-
   beneficial, core to the app, user-initiated or user-perceptible, stoppable
   by the user, and limited to the necessary duration.

### Engineering consequences for MMP

* A continuous user-enabled MMP relay is a **candidate** for the Android
  `connectedDevice` foreground-service type because it maintains a Bluetooth
  interaction. The consuming app, not the Rust crate, must make the final
  type/permission mapping with current Android and Play policy. Do not use
  `dataSync` merely as a way to obtain indefinite background relaying.
* The relay must have an explicit user action, a persistent notification that
  explains nearby-device work and battery impact, and a visible stop action.
  Defaulting a newly installed app to an always-on relay is a review and user-
  trust risk. If the relay is not user-enabled, use the MMP leaf tier.
* Request only the Bluetooth runtime permissions required by the selected
  role. Do not add location permissions or `neverForLocation` until the
  product confirms whether RSSI/proximity data is used as physical-location
  information. This is a privacy/policy decision, not a codec decision.
* Treat `AlarmManager` wakeups as opportunistic maintenance. The current
  integration guide's approximately 15-minute heartbeat is a scheduling
  target, not a platform guarantee; the documented nine-minute minimum limit
  and Doze maintenance windows still apply. No protocol delivery SLA may rely
  on a particular alarm interval.
* Keep all protocol time supplied from `SystemClock.elapsedRealtime()` and
  persist on service lifecycle transitions. A service restart must reconstruct
  `MmpNode` from the Keystore-backed seed and store path, then surface any
  persistence failure instead of silently claiming relay continuity.

### Android evidence required before a store submission

The consuming app should retain, for a target-API-34-or-newer build:

1. The merged manifest showing the service's `android:foregroundServiceType`,
   `FOREGROUND_SERVICE`, and type-specific permission, plus the exact runtime
   Bluetooth grants for central/peripheral operation.
2. A screen recording and operator log showing the user starts Relay mode,
   the notification appears, the service can be stopped, and the UI reports a
   transition to Leaf when the service is unavailable.
3. A Doze test on physical devices using the Android-documented `adb` idle
   controls. Record `tick()` wakeups, BLE callbacks, `flush()`/store results,
   message/receipt state, battery statistics, and service-stop/restart behavior.
   Repeat on representative OEM builds; stock-AOSP results are not an OEM
   compatibility claim.
4. The Play Console FGS declaration material: use-case text, deferred/
   interrupted behavior, and the demonstration video. This is submission
   evidence only; Google Play decides approval.

## 3. iOS: CoreBluetooth background modes and state restoration

### Platform facts (official documentation)

1. CoreBluetooth background execution modes are declared through
   `UIBackgroundModes`: `bluetooth-central` for central work and
   `bluetooth-peripheral` for peripheral work. The current Core Bluetooth
   documentation also requires `NSBluetoothAlwaysUsageDescription` for apps
   linked on or after iOS 13.
2. Apple documents that background central discoveries may be coalesced and
   take longer, and background peripheral advertising may omit the local name,
   move service UUIDs to an overflow area discoverable only by an iOS scanner
   explicitly scanning for those services, and reduce advertising frequency.
   Background execution is event-driven and resource-limited; it is not an
   always-running timer.
3. State preservation/restoration is opt-in. The app must create managers with
   stable unique restoration identifiers, reinstantiate the appropriate
   managers after relaunch, and implement
   `centralManager(_:willRestoreState:)` and/or
   `peripheralManager(_:willRestoreState:)`. These restoration callbacks are
   the first relevant callbacks when a preserved Bluetooth task relaunches the
   app.
4. Apple TN3115 adds a current iOS 26/iPadOS 26 caveat: Bluetooth state
   restoration relaunches are conditional, and starting in iOS 26 only apps
   that use AccessorySetupKit to set up Bluetooth accessories are relaunched
   under the documented condition. User force-quit, Bluetooth power changes,
   and pending-task state also affect relaunch eligibility. This is a platform
   rule, not something the Rust core can override.
5. App Review Guideline 2.5.4 limits multitasking apps to background services
   used for their intended purposes. It is a review rule, not a technical API
   guarantee and not legal advice.

### Engineering consequences for MMP

* The iOS app must declare only the CoreBluetooth background modes it actually
  implements and include the Bluetooth usage description. A library cannot add
  these keys on behalf of a consuming app.
* The restoration path must recreate `MmpNode` from the Keychain seed and
  private store, then attach restored central/peripheral manager state before
  accepting data. `willRestoreState` must be treated as a first-class drive-loop
  entry point, not as an optional UI callback.
* The current product wording that a “pocketed iPhone” is reachable through
  state restoration is **not an iOS 26 guarantee**. Before making that promise,
  the app team must determine whether MMP peers qualify for AccessorySetupKit,
  implement the supported setup flow if applicable, and test the exact iOS
  versions. Otherwise the honest product mode is foreground bursts plus a
  best-effort background leaf, with no guaranteed background dial-in.
* Do not use the app switcher force-quit as a successful restoration test.
  Apple documents that a user force-quit is a no-relaunch condition. Test OS
  memory termination/crash/restart separately from deliberate force-quit and
  surface the distinction in the UX.
* Keep the iOS drive loop short and event-driven. Persist on background
  transition, drain all pending protocol output while CoreBluetooth permits it,
  and reconnect from restored manager state rather than assuming a process or
  timer survives.

### iOS evidence required before a store submission

1. Archive the signed app's `Info.plist`/capabilities, Bluetooth usage text,
   restoration identifiers, and the exact iOS SDK/deployment target.
2. On physical iOS devices, record foreground discovery, background discovery,
   peripheral advertising, notification delivery, process memory termination,
   restoration delegate order, Keychain-seed reload, store recovery, and
   message/receipt outcomes.
3. Run separate cases for: app suspended, OS-terminated, crashed, user
   force-quit, Bluetooth toggled, device rebooted/first unlock, and iOS 26
   AccessorySetupKit eligibility. Mark “not relaunched” as an expected
   platform result where the official rule says so, not as a protocol failure.
4. Preserve Xcode device logs and a per-case timeline. A successful simulator
   build or XCFramework structural check is not CoreBluetooth evidence.

## 4. Release decision and open gates

E8-S3 should be reported as **dossier complete / implementation and store
submission gates open**. The Rust artifacts can be released as integration
inputs, but the project must not claim “Android background relay compliant,”
“iOS background dial-in supported,” or “store approved” until a consuming app
has produced the evidence above.

The highest-risk follow-up is the iOS 26 restoration rule: resolve the
AccessorySetupKit question before investing in UX or performance claims that
depend on automatic relaunch. The Android follow-up is the app-specific FGS
classification and user-start/stop flow, followed by API 34+ and OEM Doze
testing.

## 5. Official sources

The following are official Android, Google Play, and Apple documentation. URLs
were fetched and checked on 2026-08-09; re-check them before a real submission
because platform policy and OS behavior change independently of MMP versions.

| Source | Use in this dossier |
| --- | --- |
| https://developer.android.com/develop/background-work/services/fgs | Android foreground-service overview and user-visible notification model |
| https://developer.android.com/develop/background-work/services/fgs/service-types | Android 14+ service-type/permission rules and `connectedDevice` Bluetooth mapping |
| https://developer.android.com/develop/connectivity/bluetooth/bt-permissions | Android 12+ runtime BLE permissions and `neverForLocation` caveat |
| https://developer.android.com/training/monitoring-device-state/doze-standby | Doze/App Standby restrictions, alarm limits, and testing guidance |
| https://support.google.com/googleplay/android-developer/answer/13392821 | Play Console FGS declarations, use-case/user-impact/video requirements |
| https://developer.apple.com/documentation/CoreBluetooth?changes=_3 | Current CoreBluetooth API, usage-description, and platform notes |
| https://developer.apple.com/documentation/Xcode/configuring-background-execution-modes?changes=__8 | `UIBackgroundModes` capability configuration and supported values |
| https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html | CoreBluetooth background behavior and state preservation/restoration procedure |
| https://developer.apple.com/documentation/technotes/tn3115-bluetooth-state-restoration-app-relaunch-rules?changes=_1 | iOS 26 relaunch conditions, force-quit and Bluetooth-state caveats |
| https://developer.apple.com/app-store/review/guidelines/ | App Review Guideline 2.5.4 boundary for background services |

