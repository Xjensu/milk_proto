// Protocol-neutral lifecycle boundary for an iOS CoreBluetooth adapter.
//
// CoreBluetooth objects are intentionally not referenced here.  A native
// adapter creates one MeshConnectionIdentity for each physical connection,
// captures the resulting MeshConnectionBinding in every delegate callback,
// and forwards those callbacks to MeshRadioShell on its serial drive queue.

public enum MeshRadioRole: Equatable {
    case central
    case peripheral

    fileprivate var initiator: Bool {
        switch self {
        case .central:
            return true
        case .peripheral:
            return false
        }
    }
}

/// The identifiers passed to CBCentralManager/CBPeripheralManager restoration
/// options.  They are part of the host integration contract, not wire data.
public enum MeshRestorationIdentity {
    public static let central = "mmp.central"
    public static let peripheral = "mmp.peripheral"
}

public struct MeshRestorationState: Equatable {
    public let central: Bool
    public let peripheral: Bool

    public init(central: Bool = false, peripheral: Bool = false) {
        self.central = central
        self.peripheral = peripheral
    }
}

public enum MeshRadioShellError: Error, Equatable {
    case alreadyRunning
    case linkTokenExhausted
}

/// Opaque identity for one physical GATT connection instance.
///
/// A fresh object is required for every reconnect.  Native handles and
/// peripheral identifiers can be reused while delayed callbacks for an older
/// connection are still queued, so they are deliberately not accepted as the
/// identity key.
public final class MeshConnectionIdentity {
    private init() {}

    public static func create() -> MeshConnectionIdentity {
        MeshConnectionIdentity()
    }
}

/// Process-local, monotonic link-token source for the protocol shell.
public final class MeshLinkTokenAllocator {
    private var nextToken: UInt64

    public init(firstToken: UInt64 = 1) {
        precondition(firstToken > 0, "link tokens must be positive")
        nextToken = firstToken
    }

    /// Tokens are never reused and the maximum value is reserved to prevent
    /// an increment from wrapping into an invalid/old identity.
    public func allocate() throws -> UInt64 {
        guard nextToken < UInt64.max else {
            throw MeshRadioShellError.linkTokenExhausted
        }
        let token = nextToken
        nextToken += 1
        return token
    }
}

/// Immutable callback context for one physical connection instance.
public final class MeshConnectionBinding {
    public let link: UInt64
    public let role: MeshRadioRole

    fileprivate let identity: MeshConnectionIdentity

    fileprivate init(
        identity: MeshConnectionIdentity,
        link: UInt64,
        role: MeshRadioRole
    ) {
        self.identity = identity
        self.link = link
        self.role = role
    }
}

/// Binds native connection-instance identities to coordinator link tokens.
///
/// The registry is deliberately single-owner and unsynchronized: all methods
/// must be called from the same serial queue that owns MeshDrive.  This makes
/// callback ordering explicit and avoids pretending that CoreBluetooth
/// delegate delivery is safe for concurrent protocol calls.
public final class MeshConnectionBindingRegistry {
    private let maxBindings: Int
    private let allocator: MeshLinkTokenAllocator
    private var active: [ObjectIdentifier: MeshConnectionBinding] = [:]

    public init(
        maxBindings: Int = 64,
        allocator: MeshLinkTokenAllocator = MeshLinkTokenAllocator()
    ) {
        precondition(maxBindings > 0, "maxBindings must be positive")
        self.maxBindings = maxBindings
        self.allocator = allocator
    }

    public var count: Int { active.count }

    /// Return the existing binding for a duplicate callback, or allocate one
    /// only when this identity is first admitted.
    public func open(
        _ identity: MeshConnectionIdentity,
        role: MeshRadioRole
    ) throws -> MeshConnectionBinding? {
        let key = ObjectIdentifier(identity)
        if let existing = active[key] {
            return existing
        }
        guard active.count < maxBindings else { return nil }
        let binding = MeshConnectionBinding(
            identity: identity,
            link: try allocator.allocate(),
            role: role
        )
        active[key] = binding
        return binding
    }

    public func binding(for identity: MeshConnectionIdentity) -> MeshConnectionBinding? {
        active[ObjectIdentifier(identity)]
    }

    /// Retire only the exact binding that still owns its identity.
    @discardableResult
    public func close(_ binding: MeshConnectionBinding) -> Bool {
        let key = ObjectIdentifier(binding.identity)
        guard active[key] === binding else { return false }
        active.removeValue(forKey: key)
        return true
    }

    public func isCurrent(_ binding: MeshConnectionBinding) -> Bool {
        active[ObjectIdentifier(binding.identity)] === binding
    }

    fileprivate func removeAll() {
        active.removeAll(keepingCapacity: true)
    }
}

/// CoreBluetooth-neutral iOS shell for MeshDrive.
///
/// The shell does not create a radio, sleep, or emulate background execution.
/// It translates central/peripheral delegate ordering into one identity-bound
/// MeshDrive link lifecycle and leaves actual GATT writes/notifications to the
/// injected GattAdapter.  All calls must be serialized by the native adapter.
public final class MeshRadioShell {
    private enum Lifecycle: Equatable {
        case stopped
        case starting
        case running
    }

    private let drive: MeshDrive
    private let registry: MeshConnectionBindingRegistry
    private var lifecycle: Lifecycle = .stopped
    private var restoration = MeshRestorationState()

    public init(
        drive: MeshDrive,
        registry: MeshConnectionBindingRegistry = MeshConnectionBindingRegistry()
    ) {
        self.drive = drive
        self.registry = registry
    }

    public var isRunning: Bool { lifecycle != .stopped && drive.isRunning }
    public var activeLinkCount: Int { registry.count }
    public var restorationState: MeshRestorationState { restoration }

    public func start() throws {
        guard lifecycle == .stopped else {
            throw MeshRadioShellError.alreadyRunning
        }
        lifecycle = .starting
        do {
            try drive.start()
            lifecycle = .running
        } catch {
            registry.removeAll()
            lifecycle = .stopped
            throw error
        }
    }

    public func stop() {
        guard lifecycle != .stopped else {
            registry.removeAll()
            return
        }
        drive.stop()
        registry.removeAll()
        lifecycle = .stopped
    }

    /// Forward an explicit host lifecycle flush without coupling persistence
    /// to radio teardown. This remains callable after `stop()` for termination
    /// hooks that release the radio before asking the native adapter to flush.
    public func flushPersistence() throws {
        try drive.flushPersistence()
    }

    /// Record the exact callback identity restored by CoreBluetooth.  A
    /// mismatched identifier is rejected rather than silently treated as a
    /// restored controller.
    @discardableResult
    public func onRestored(_ role: MeshRadioRole, identifier: String) -> Bool {
        switch role {
        case .central where identifier == MeshRestorationIdentity.central:
            restoration = MeshRestorationState(central: true, peripheral: restoration.peripheral)
            return true
        case .peripheral where identifier == MeshRestorationIdentity.peripheral:
            restoration = MeshRestorationState(central: restoration.central, peripheral: true)
            return true
        default:
            return false
        }
    }

    /// Forward a central didDiscover callback only while the drive is alive.
    public func onScanResult(_ result: ScanResult) {
        guard isRunning else { return }
        drive.onScanResult(result)
    }

    /// Admit central didConnect or peripheral didSubscribe in that order.
    /// A nil result means the native adapter must close the physical link.
    public func onLinkUp(
        _ identity: MeshConnectionIdentity,
        role: MeshRadioRole,
        quality: MeshLinkQualityHint? = nil
    ) throws -> MeshConnectionBinding? {
        guard lifecycle != .stopped, drive.isRunning else { return nil }
        if let existing = registry.binding(for: identity) {
            return existing
        }
        guard let binding = try registry.open(identity, role: role) else {
            return nil
        }
        do {
            try drive.onLinkUp(binding.link, initiator: role.initiator, quality: quality)
            if !drive.isRunning || drive.isOverloaded {
                _ = registry.close(binding)
                return nil
            }
            return binding
        } catch {
            _ = registry.close(binding)
            try? drive.onLinkDown(binding.link)
            throw error
        }
    }

    /// Data after disconnect or from a stale binding is
    /// ignored.  This mirrors CoreBluetooth's late-callback behavior without
    /// allowing a retired connection to feed protocol state.
    public func onLinkData(
        _ binding: MeshConnectionBinding,
        bytes: [UInt8]
    ) throws {
        guard isRunning, registry.isCurrent(binding) else {
            return
        }
        try drive.onLinkData(binding.link, bytes: bytes)
    }

    /// Call after the native central/peripheral disconnect callback arrives.
    public func onLinkDown(_ binding: MeshConnectionBinding) throws {
        guard registry.isCurrent(binding) else { return }
        defer { _ = registry.close(binding) }
        guard isRunning else { return }
        try drive.onLinkDown(binding.link)
    }

    /// CoreBluetooth invokes this when write-without-response capacity returns.
    public func onReadyToSend(_ binding: MeshConnectionBinding) throws {
        guard isRunning, registry.isCurrent(binding) else { return }
        try drive.pump()
    }

    public func pump() throws {
        guard isRunning else { return }
        try drive.pump()
    }

    public func pumpAt(_ nowMs: UInt64) throws {
        guard isRunning else { return }
        try drive.pumpAt(nowMs)
    }

    /// Background iOS is a leaf; only an explicitly available foreground
    /// capability may opt into relay behavior.
    public func setForegroundRelayCapable(_ capable: Bool) {
        drive.setForegroundRelayCapable(capable)
    }
}
