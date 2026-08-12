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

public struct ScanResult {
    public let serviceData: [UInt8]?
    public let rssi: Int8

    public init(serviceData: [UInt8]?, rssi: Int8) {
        self.serviceData = serviceData
        self.rssi = rssi
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

public protocol MeshNode: AnyObject {
    func setTier(_ tier: MeshTier)
    func beacon(nowMs: UInt64) -> [UInt8]
    func tick(nowMs: UInt64)
    func pollClose() -> UInt64?
    func closeActionsDropped() -> UInt64
    func eventQueueDropped() -> UInt64
    func pollSend() -> OutboundFrame?
    func pollEvent() -> Any?
    func onBeacon(serviceData: [UInt8], rssi: Int8)
    func linkUp(_ link: UInt64, initiator: Bool)
    /// Optional measured transport quality.  The default delegates to the
    /// legacy callback so existing FFI adapters remain source-compatible.
    func linkUp(_ link: UInt64, initiator: Bool, quality: MeshLinkQualityHint?)
    func linkData(_ link: UInt64, bytes: [UInt8])
    func linkDown(_ link: UInt64)
}

public extension MeshNode {
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
    private let eventSink: (Any) -> Void
    private var running = false
    private var relayCapable: Bool
    private var lastTickMs: UInt64?
    private var advertisedBeacon: [UInt8]?
    private var activeLinks = Set<UInt64>()
    private let maxTrackedLinks = 64
    private var closeRequested = Set<UInt64>()
    private var pending: [PendingFrame] = []
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
        eventSink: @escaping (Any) -> Void = { _ in }
    ) {
        self.node = node
        self.clock = clock
        self.advertiser = advertiser
        self.scanner = scanner
        self.gatt = gatt
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
            precondition(running, "MeshDrive failed closed during startup")
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
        try? scanner.stop()
        try? advertiser.stop()
        advertisedBeacon = nil
        running = false
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
        guard running, let payload = result.serviceData, !payload.isEmpty else { return }
        node.onBeacon(serviceData: payload, rssi: result.rssi)
    }

    public func onLinkUp(
        _ link: UInt64,
        initiator: Bool,
        quality: MeshLinkQualityHint? = nil
    ) throws {
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
        if activeLinks.contains(link) { return }
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
        try drain()
    }

    public func onLinkData(_ link: UInt64, bytes: [UInt8]) throws {
        guard running, !overloadLatched, activeLinks.contains(link), !bytes.isEmpty else { return }
        node.linkData(link, bytes: bytes)
        try drain()
    }

    public func onLinkDown(_ link: UInt64) throws {
        closeRequested.remove(link)
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
        drainEvents()
    }

    private func drainClose() throws {
        while let link = node.pollClose() {
            pending.removeAll { $0.link == link }
            try requestPhysicalClose(link)
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
        links.forEach { try? requestPhysicalClose($0) }
        try? scanner.stop()
        try? advertiser.stop()
        advertisedBeacon = nil
        running = false
    }

    private func drainSend() throws {
        while true {
            if pending.isEmpty {
                guard let next = node.pollSend() else { return }
                guard !next.bytes.isEmpty else { continue }
                pending.append(PendingFrame(link: next.link, bytes: next.bytes, fragmentSize: nil, offset: 0))
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

    private func drainEvents() {
        while let event = node.pollEvent() { eventSink(event) }
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
        advertisedBeacon = nil
    }

    private func requireRunning() throws {
        guard running else { throw MeshDriveError.notRunning }
    }
}

public enum MeshDriveError: Error, Equatable {
    case notRunning
    case clockMovedBackwards
    case invalidMtu(Int)
    case invalidLinkQuality(mtu: Int, goodputBps: Int)
}
