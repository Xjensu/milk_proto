// Protocol-neutral iOS host coordinator for the MMP/1 drive loop.
//
// This file intentionally imports no Apple framework.  An iOS application
// supplies the CoreBluetooth adapter and calls the callbacks from one serial
// queue.  The coordinator owns ordering, bounded host state, and backpressure;
// it does not own a radio, a clock, or a Rust node implementation.

public enum MeshTier {
    case leaf
    case relay
}

/// Development service UUID used by the native BLE adapter for MMP beacons.
///
/// The value is transport metadata, not wire payload.  A production SIG
/// assignment remains a release gate; keeping the value in this SDK-free
/// contract prevents platform adapters from silently drifting apart.
public enum MmpBleService {
    public static let serviceUUID = "6d6d7031-0000-4d4d-5031-6d6573686d6c"
}

public struct ScanResult {
    public let serviceData: [UInt8]?
    public let rssi: Int8
    /// UUID observed by the native scanner for the service-data payload.
    /// Missing or mismatched values are rejected before `MeshNode.onBeacon`.
    public let serviceUuid: String?

    public init(serviceData: [UInt8]?, rssi: Int8, serviceUuid: String? = nil) {
        self.serviceData = serviceData
        self.rssi = rssi
        self.serviceUuid = serviceUuid
    }
}

public struct OutboundFrame {
    public let link: UInt64
    public let bytes: [UInt8]

    public init(link: UInt64, bytes: [UInt8]) {
        self.link = link
        self.bytes = bytes
    }
}

/// Local transport measurements used by the Rust scheduler.  The record is
/// never serialized into an MMP/1 frame; admission validates it before a
/// physical callback can create a logical protocol session.
public struct MeshLinkQualityHint: Equatable {
    public let mtu: Int
    public let goodputBps: Int

    public init(mtu: Int, goodputBps: Int) {
        self.mtu = mtu
        self.goodputBps = goodputBps
    }

    public var isValid: Bool {
        (Self.minMtu...Self.maxMtu).contains(mtu)
            && (Self.minGoodputBps...Self.maxGoodputBps).contains(goodputBps)
    }

    public static let minMtu = 23
    public static let maxMtu = 4_096
    public static let minGoodputBps = 1
    public static let maxGoodputBps = 10_000_000
}

/// Host-only BLE scheduling configuration. This is transport-local state: it
/// is not encoded into an MMP/1 frame and this coordinator never starts a
/// timer from it. The native adapter owns the actual radio scheduling and
/// enforces the forwarded session-airtime bound.
public struct MeshDutyCycleRequest: Equatable {
    public let baseAdvertisementIntervalMs: Int
    public let advertisementJitterPercent: Int
    public let scanWindowMs: Int
    public let scanIntervalMs: Int
    public let burstScanDurationMs: Int
    public let burstScanMaxPerMinute: Int
    public let connectionIntervalMs: Int
    public let supervisionTimeoutMs: Int
    public let perSessionAirtimeCapMs: Int

    public init(
        baseAdvertisementIntervalMs: Int = 5_000,
        advertisementJitterPercent: Int = 10,
        scanWindowMs: Int = 30,
        scanIntervalMs: Int = 1_280,
        burstScanDurationMs: Int = 30_000,
        burstScanMaxPerMinute: Int = 1,
        connectionIntervalMs: Int = 30,
        supervisionTimeoutMs: Int = 4_000,
        perSessionAirtimeCapMs: Int = 60_000
    ) {
        self.baseAdvertisementIntervalMs = baseAdvertisementIntervalMs
        self.advertisementJitterPercent = advertisementJitterPercent
        self.scanWindowMs = scanWindowMs
        self.scanIntervalMs = scanIntervalMs
        self.burstScanDurationMs = burstScanDurationMs
        self.burstScanMaxPerMinute = burstScanMaxPerMinute
        self.connectionIntervalMs = connectionIntervalMs
        self.supervisionTimeoutMs = supervisionTimeoutMs
        self.perSessionAirtimeCapMs = perSessionAirtimeCapMs
    }

    public func validate() throws {
        guard (2_000...10_000).contains(baseAdvertisementIntervalMs) else {
            throw MeshDutyCycleError.invalidAdvertisementInterval
        }
        guard advertisementJitterPercent == 10 else {
            throw MeshDutyCycleError.invalidAdvertisementJitter
        }
        guard (30...60_000).contains(scanIntervalMs) else {
            throw MeshDutyCycleError.invalidScanInterval
        }
        guard (30...scanIntervalMs).contains(scanWindowMs) else {
            throw MeshDutyCycleError.invalidScanWindow
        }
        guard scanWindowMs <= scanIntervalMs / 40 else {
            throw MeshDutyCycleError.invalidScanDuty
        }
        guard (0...30_000).contains(burstScanDurationMs) else {
            throw MeshDutyCycleError.invalidBurstDuration
        }
        guard (0...6).contains(burstScanMaxPerMinute) else {
            throw MeshDutyCycleError.invalidBurstRate
        }
        guard (burstScanDurationMs == 0) == (burstScanMaxPerMinute == 0) else {
            throw MeshDutyCycleError.invalidBurstConfiguration
        }
        guard (15...30).contains(connectionIntervalMs) else {
            throw MeshDutyCycleError.invalidConnectionInterval
        }
        guard supervisionTimeoutMs == 4_000 else {
            throw MeshDutyCycleError.invalidSupervisionTimeout
        }
        guard perSessionAirtimeCapMs == 60_000 else {
            throw MeshDutyCycleError.invalidSessionAirtimeCap
        }
    }
}

public enum MeshDutyCycleError: Error, Equatable {
    case invalidAdvertisementInterval
    case invalidAdvertisementJitter
    case invalidScanWindow
    case invalidScanDuty
    case invalidScanInterval
    case invalidBurstDuration
    case invalidBurstRate
    case invalidBurstConfiguration
    case invalidConnectionInterval
    case invalidSupervisionTimeout
    case invalidSessionAirtimeCap
}

/// Adapter-facing sink for transport-local scheduling configuration.
public protocol DutyCycleSink {
    func applyDutyCycle(_ request: MeshDutyCycleRequest) throws
}

public struct NoopDutyCycleSink: DutyCycleSink {
    public init() {}

    public func applyDutyCycle(_ request: MeshDutyCycleRequest) throws {}
}

/// Host-owned persistence boundary for the existing mmp-ffi snapshot store.
/// The coordinator never invokes this callback automatically and never sees
/// keys or snapshot bytes. A native adapter should wrap `MmpNode.flush()` and
/// call `MeshDrive.flushPersistence()` at an explicit lifecycle boundary.
public protocol PersistenceSink {
    func flushPersistence() throws
}

public struct NoopPersistenceSink: PersistenceSink {
    public init() {}

    public func flushPersistence() throws {}
}

/// Host-owned boundary for Rust's keystore-backed secret-state vault.
///
/// The native adapter performs the vault transaction around its `MmpNode`.
/// Only the durable generation crosses this coordinator boundary: ciphertext,
/// wrapping keys, chain keys, prekeys, and plaintext snapshots never do. A
/// `nil` restore generation means a genuinely new provider has no committed
/// record. Provider-unavailable, locked, invalid, busy, or rollback-rejected
/// state must be reported by throwing rather than being treated as a fresh
/// node.
public protocol SecretStatePersistence {
    /// Restore optional secret continuity before radio startup.
    func restoreSecretState() throws -> UInt64?

    /// Commit secret continuity and return the newly durable generation.
    func persistSecretState() throws -> UInt64
}

/// Explicit fail-closed default. This is not an in-memory vault and never
/// pretends that secret continuity exists without a native capability.
public struct UnavailableSecretStatePersistence: SecretStatePersistence {
    public init() {}

    public func restoreSecretState() throws -> UInt64? {
        throw MeshSecretStateError.unavailable
    }

    public func persistSecretState() throws -> UInt64 {
        throw MeshSecretStateError.unavailable
    }
}

public enum MeshSecretStateError: Error, Equatable {
    case unavailable
    case invalidGeneration
    /// Secret continuity may only be restored before radio traffic starts.
    case invalidLifecycle
}

private let meshNodeMaxPollBatch = 64

public protocol MeshNode: AnyObject {
    func setTier(_ tier: MeshTier)
    func beacon(nowMs: UInt64) -> [UInt8]
    func tick(nowMs: UInt64)
    func pollClose() -> UInt64?
    /// Drain close actions in FIFO order; existing adapters use the fallback.
    func pollCloseBatch(_ maxItems: Int) -> [UInt64]
    func closeActionsDropped() -> UInt64
    func eventQueueDropped() -> UInt64
    func pollSend() -> OutboundFrame?
    /// Drain outbound frames in FIFO order; existing adapters use the fallback.
    func pollSendBatch(_ maxItems: Int) -> [OutboundFrame]
    func pollEvent() -> Any?
    /// Drain application events in FIFO order; existing adapters use the fallback.
    func pollEventBatch(_ maxItems: Int) -> [Any]
    func onBeacon(serviceData: [UInt8], rssi: Int8)
    func linkUp(_ link: UInt64, initiator: Bool)
    /// Optional measured transport quality.  The default delegates to the
    /// legacy callback so existing FFI adapters remain source-compatible.
    func linkUp(_ link: UInt64, initiator: Bool, quality: MeshLinkQualityHint?)
    func linkData(_ link: UInt64, bytes: [UInt8])
    func linkDown(_ link: UInt64)
}

public extension MeshNode {
    func pollCloseBatch(_ maxItems: Int) -> [UInt64] {
        let limit = max(0, min(maxItems, meshNodeMaxPollBatch))
        guard limit > 0 else { return [] }
        var values: [UInt64] = []
        values.reserveCapacity(limit)
        while values.count < limit, let link = pollClose() {
            values.append(link)
        }
        return values
    }

    func pollSendBatch(_ maxItems: Int) -> [OutboundFrame] {
        let limit = max(0, min(maxItems, meshNodeMaxPollBatch))
        guard limit > 0 else { return [] }
        var values: [OutboundFrame] = []
        values.reserveCapacity(limit)
        while values.count < limit, let frame = pollSend() {
            values.append(frame)
        }
        return values
    }

    func pollEventBatch(_ maxItems: Int) -> [Any] {
        let limit = max(0, min(maxItems, meshNodeMaxPollBatch))
        guard limit > 0 else { return [] }
        var values: [Any] = []
        values.reserveCapacity(limit)
        while values.count < limit, let event = pollEvent() {
            values.append(event)
        }
        return values
    }

    func linkUp(_ link: UInt64, initiator: Bool, quality: MeshLinkQualityHint?) {
        linkUp(link, initiator: initiator)
    }
}

public protocol MonotonicClock {
    func nowMs() -> UInt64
}

public protocol Advertiser {
    func start(serviceData: [UInt8]) throws
    func stop() throws
}

public protocol Scanner {
    func start(onResult: @escaping (ScanResult) -> Void) throws
    func stop() throws
}

public protocol GattAdapter {
    func mtu(link: UInt64) throws -> Int
    func notifyFragment(link: UInt64, payload: [UInt8]) throws -> Bool
    func close(link: UInt64) throws
}

/// Single-owner iOS host coordinator.  Callbacks and `pump` must be serialized
/// by the host's dispatch queue, just as the native CoreBluetooth delegates
/// are serialized in the consuming application.
public final class MeshDrive {
    public static let attHeaderBytes = 3
    public static let minAttMtu = 23

    private struct PendingFrame {
        let link: UInt64
        let bytes: [UInt8]
        var fragmentSize: Int?
        var offset: Int
    }

    private let node: MeshNode
    private let clock: MonotonicClock
    private let advertiser: Advertiser
    private let scanner: Scanner
    private let gatt: GattAdapter
    private let dutyCycleRequest: MeshDutyCycleRequest
    private let dutyCycleSink: DutyCycleSink
    private let persistenceSink: PersistenceSink
    private let secretStatePersistence: SecretStatePersistence
    private let eventSink: (Any) throws -> Void
    private var running = false
    // Persisting before a successful radio lifecycle could overwrite a
    // durable record with an uninitialized secret snapshot.
    private var hasCompletedStart = false
    private var relayCapable: Bool
    private var lastTickMs: UInt64?
    private var advertisedBeacon: [UInt8]?
    private var activeLinks = Set<UInt64>()
    private let maxTrackedLinks = 64
    private var closeRequested = Set<UInt64>()
    private var pending: [PendingFrame] = []
    // Batch polling consumes native queue entries before the platform sink
    // accepts them. Keep the unprocessed suffix locally so a sink exception
    // or back-pressure cannot reorder or discard close/event work.
    private var pendingCloseActions: [UInt64] = []
    private var pendingEvents: [Any] = []
    private var overloadLatched = false
    private var observedCloseDrops: UInt64 = 0
    private var observedEventDrops: UInt64 = 0

    public init(
        node: MeshNode,
        clock: MonotonicClock,
        advertiser: Advertiser,
        scanner: Scanner,
        gatt: GattAdapter,
        foregroundRelayCapable: Bool,
        eventSink: @escaping (Any) throws -> Void = { _ in },
        dutyCycleRequest: MeshDutyCycleRequest = MeshDutyCycleRequest(),
        dutyCycleSink: DutyCycleSink = NoopDutyCycleSink(),
        persistenceSink: PersistenceSink = NoopPersistenceSink(),
        secretStatePersistence: SecretStatePersistence = UnavailableSecretStatePersistence()
    ) {
        self.node = node
        self.clock = clock
        self.advertiser = advertiser
        self.scanner = scanner
        self.gatt = gatt
        self.dutyCycleRequest = dutyCycleRequest
        self.dutyCycleSink = dutyCycleSink
        self.persistenceSink = persistenceSink
        self.secretStatePersistence = secretStatePersistence
        self.relayCapable = foregroundRelayCapable
        self.eventSink = eventSink
    }

    public var isRunning: Bool { running }
    public var lastTick: UInt64? { lastTickMs }
    public var isOverloaded: Bool { overloadLatched }
    public var closeActionsDropped: UInt64 { node.closeActionsDropped() }
    public var eventQueueDropped: UInt64 { node.eventQueueDropped() }

    public func start() throws {
        precondition(!running, "MeshDrive is already running")
        precondition(!overloadLatched, "MeshDrive is latched after host overload")
        try dutyCycleRequest.validate()
        // Apply once before any native radio adapter starts. Swift structs have
        // value semantics, so the sink cannot retain mutable coordinator state.
        try dutyCycleSink.applyDutyCycle(dutyCycleRequest)
        running = true
        var advertiserStarted = false
        var scannerStarted = false
        do {
            applyTier()
            let now = clock.nowMs()
            try advance(nowMs: now, emit: false)
            let beacon = node.beacon(nowMs: now)
            do {
                try advertiser.start(serviceData: beacon)
            } catch {
                // A native adapter may allocate state before reporting failure.
                // Always release that state, while preserving the start error.
                try? advertiser.stop()
                throw error
            }
            advertisedBeacon = beacon
            advertiserStarted = true
            do {
                try scanner.start { [weak self] result in
                    self?.onScanResult(result)
                }
            } catch {
                // Keep startup failure deterministic even when start partially
                // initialized the native scanner before throwing.
                try? scanner.stop()
                throw error
            }
            scannerStarted = true
            try drain()
            guard running else {
                throw MeshDriveError.hostOverload
            }
            hasCompletedStart = true
        } catch {
            if scannerStarted { try? scanner.stop() }
            if advertiserStarted { try? advertiser.stop() }
            rollbackStartLinks()
            running = false
            throw error
        }
    }

    public func stop() {
        guard running else { return }
        let links = Array(activeLinks)
        links.forEach { node.linkDown($0) }
        activeLinks.removeAll()
        links.forEach { try? requestPhysicalClose($0) }
        closeRequested.removeAll()
        pending.removeAll()
        pendingCloseActions.removeAll()
        pendingEvents.removeAll()
        try? scanner.stop()
        try? advertiser.stop()
        advertisedBeacon = nil
        running = false
    }

    /// Flush host-owned durable state at an explicit lifecycle boundary.
    ///
    /// This method is valid while the radio is running or stopped, does not
    /// stop/start any adapter, and is never called implicitly by `start()` or
    /// `stop()`. The native host owns the actual `MmpNode.flush()` call and
    /// Keystore/Keychain policy; errors are propagated unchanged.
    public func flushPersistence() throws {
        try persistenceSink.flushPersistence()
    }

    /// Restore secret continuity explicitly at a host lifecycle boundary.
    /// This is intentionally not called by `start()` or `stop()`; the native
    /// adapter should invoke it after constructing the Rust node and before
    /// admitting radio traffic.
    public func restoreSecretState() throws -> UInt64? {
        // Restoring after startup could replace the continuity state while
        // links are already admitted.  Fail closed instead of allowing a
        // native adapter to create a split-brain session lifecycle.
        guard !running else {
            throw MeshSecretStateError.invalidLifecycle
        }
        let generation = try secretStatePersistence.restoreSecretState()
        if let generation, generation == 0 {
            throw MeshSecretStateError.invalidGeneration
        }
        return generation
    }

    /// Persist secret continuity explicitly. Provider failures propagate, and
    /// a zero generation can never be reported as a successful commit. The
    /// operation is serialized at the lifecycle boundary after radio teardown,
    /// matching the Rust vault's no-live-session restore invariant.
    public func persistSecretState() throws -> UInt64 {
        guard !running else {
            throw MeshSecretStateError.invalidLifecycle
        }
        guard hasCompletedStart else {
            throw MeshSecretStateError.invalidLifecycle
        }
        let generation = try secretStatePersistence.persistSecretState()
        guard generation > 0 else {
            throw MeshSecretStateError.invalidGeneration
        }
        return generation
    }

    public func pump() throws {
        try requireRunning()
        try advance(nowMs: clock.nowMs(), emit: true)
    }

    public func pumpAt(_ nowMs: UInt64) throws {
        try requireRunning()
        try advance(nowMs: nowMs, emit: true)
    }

    public func refreshBeacon(nowMs: UInt64) throws {
        try requireRunning()
        try requireMonotonic(nowMs)
        let beacon = node.beacon(nowMs: nowMs)
        guard advertisedBeacon != beacon else { return }
        try advertiser.start(serviceData: beacon)
        advertisedBeacon = beacon
    }

    public func setForegroundRelayCapable(_ capable: Bool) {
        relayCapable = capable
        applyTier()
    }

    public func onScanResult(_ result: ScanResult) {
        guard running,
              let observedServiceUuid = result.serviceUuid,
              observedServiceUuid.lowercased() == MmpBleService.serviceUUID,
              let payload = result.serviceData,
              !payload.isEmpty
        else { return }
        node.onBeacon(serviceData: payload, rssi: result.rssi)
    }

    public func onLinkUp(
        _ link: UInt64,
        initiator: Bool,
        quality: MeshLinkQualityHint? = nil
    ) throws {
        // Duplicate callbacks are idempotent.  Do not let a malformed second
        // quality sample tear down an already-admitted physical link.
        if activeLinks.contains(link) { return }
        if let quality, !quality.isValid {
            // Reject before activeLinks or the Rust session is touched.  The
            // native callback is still torn down on malformed measurements.
            try? gatt.close(link: link)
            throw MeshDriveError.invalidLinkQuality(
                mtu: quality.mtu,
                goodputBps: quality.goodputBps
            )
        }
        guard running, !overloadLatched else {
            try? gatt.close(link: link)
            return
        }
        guard activeLinks.count < maxTrackedLinks else {
            latchOverload()
            try? gatt.close(link: link)
            return
        }
        guard activeLinks.insert(link).inserted else { return }
        if let quality {
            node.linkUp(link, initiator: initiator, quality: quality)
        } else {
            node.linkUp(link, initiator: initiator)
        }
        do {
            try drain()
        } catch {
            // Admission is a transaction across the logical session and the
            // physical GATT link.  Roll both back before propagating failure.
            rollbackLinkAdmission(link)
            throw error
        }
    }

    public func onLinkData(_ link: UInt64, bytes: [UInt8]) throws {
        guard running, !overloadLatched, activeLinks.contains(link), !bytes.isEmpty else { return }
        node.linkData(link, bytes: bytes)
        try drain()
    }

    public func onLinkDown(_ link: UInt64) throws {
        closeRequested.remove(link)
        pendingCloseActions.removeAll { $0 == link }
        guard running, activeLinks.remove(link) != nil else { return }
        pending.removeAll { $0.link == link }
        node.linkDown(link)
        try drain()
    }

    private func advance(nowMs: UInt64, emit: Bool) throws {
        try requireMonotonic(nowMs)
        node.tick(nowMs: nowMs)
        lastTickMs = nowMs
        if emit { try drain() }
    }

    private func requireMonotonic(_ nowMs: UInt64) throws {
        if let previous = lastTickMs, nowMs < previous {
            throw MeshDriveError.clockMovedBackwards
        }
    }

    private func drain() throws {
        try drainClose()
        observeHostDiagnostics()
        if !overloadLatched { try drainSend() }
        try drainEvents()
    }

    private func drainClose() throws {
        while true {
            if pendingCloseActions.isEmpty {
                pendingCloseActions.append(contentsOf: node.pollCloseBatch(meshNodeMaxPollBatch))
            }
            guard !pendingCloseActions.isEmpty else { return }
            let link = pendingCloseActions.removeFirst()
            pending.removeAll { $0.link == link }
            do {
                try requestPhysicalClose(link)
            } catch {
                // The native queue entry has already been consumed. Retain
                // it only while this coordinator still owns the logical
                // link; rollback/onLinkDown paths retire stale retries.
                if activeLinks.contains(link) {
                    pendingCloseActions.insert(link, at: 0)
                }
                throw error
            }
        }
    }

    private func requestPhysicalClose(_ link: UInt64) throws {
        guard closeRequested.insert(link).inserted else { return }
        do {
            try gatt.close(link: link)
        } catch {
            closeRequested.remove(link)
            throw error
        }
    }

    private func observeHostDiagnostics() {
        let closeDrops = node.closeActionsDropped()
        let eventDrops = node.eventQueueDropped()
        let closeDelta = closeDrops > observedCloseDrops
        let eventDelta = eventDrops > observedEventDrops
        observedCloseDrops = max(observedCloseDrops, closeDrops)
        observedEventDrops = max(observedEventDrops, eventDrops)
        if closeDelta || eventDelta { latchOverload() }
    }

    private func latchOverload() {
        guard !overloadLatched else { return }
        overloadLatched = true
        let links = Array(activeLinks)
        links.forEach { node.linkDown($0) }
        activeLinks.removeAll()
        pending.removeAll()
        pendingCloseActions.removeAll()
        links.forEach { try? requestPhysicalClose($0) }
        try? scanner.stop()
        try? advertiser.stop()
        advertisedBeacon = nil
        running = false
    }

    private func drainSend() throws {
        while true {
            if pending.isEmpty {
                let batch = node.pollSendBatch(meshNodeMaxPollBatch)
                if batch.isEmpty { return }
                for next in batch where !next.bytes.isEmpty {
                    pending.append(PendingFrame(link: next.link, bytes: next.bytes, fragmentSize: nil, offset: 0))
                }
                if pending.isEmpty { continue }
            }
            guard !pending.isEmpty else { return }
            var frame = pending[0]
            if frame.fragmentSize == nil {
                let mtu = try gatt.mtu(link: frame.link)
                guard mtu >= Self.minAttMtu else { throw MeshDriveError.invalidMtu(mtu) }
                frame.fragmentSize = mtu - Self.attHeaderBytes
            }
            guard let fragmentSize = frame.fragmentSize, frame.offset <= frame.bytes.count else {
                pending.removeFirst()
                continue
            }
            let count = min(fragmentSize, frame.bytes.count - frame.offset)
            let end = frame.offset + count
            let accepted = try gatt.notifyFragment(link: frame.link, payload: Array(frame.bytes[frame.offset..<end]))
            pending[0] = frame
            guard accepted else { return }
            pending[0].offset = end
            if end == frame.bytes.count { pending.removeFirst() }
        }
    }

    private func drainEvents() throws {
        while true {
            if pendingEvents.isEmpty {
                pendingEvents.append(contentsOf: node.pollEventBatch(meshNodeMaxPollBatch))
            }
            guard !pendingEvents.isEmpty else { return }
            let event = pendingEvents.removeFirst()
            do {
                try eventSink(event)
            } catch {
                // Keep the event at the head when the host bridge rejects it.
                // This preserves FIFO and lets a later lifecycle/pump retry
                // without re-reading or duplicating the native queue entry.
                pendingEvents.insert(event, at: 0)
                throw error
            }
        }
    }

    private func applyTier() {
        node.setTier(relayCapable ? .relay : .leaf)
    }

    private func rollbackStartLinks() {
        let links = Array(activeLinks)
        links.forEach { node.linkDown($0) }
        activeLinks.removeAll()
        links.forEach { try? requestPhysicalClose($0) }
        closeRequested.removeAll()
        pending.removeAll()
        pendingCloseActions.removeAll()
        advertisedBeacon = nil
    }

    private func rollbackLinkAdmission(_ link: UInt64) {
        pendingCloseActions.removeAll { $0 == link }
        pending.removeAll { $0.link == link }
        if activeLinks.remove(link) != nil {
            node.linkDown(link)
        }
        try? requestPhysicalClose(link)
    }

    private func requireRunning() throws {
        guard running else { throw MeshDriveError.notRunning }
    }
}

public enum MeshDriveError: Error, Equatable {
    case notRunning
    case hostOverload
    case clockMovedBackwards
    case invalidMtu(Int)
    case invalidLinkQuality(mtu: Int, goodputBps: Int)
}
