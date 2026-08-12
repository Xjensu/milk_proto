import XCTest
import MeshDrive

final class MeshDriveConsumerTests: XCTestCase {
    func testRadioShellBindsEachPhysicalIdentityToOneMonotonicLink() throws {
        let node = TestNode()
        let drive = makeDrive(node: node)
        let shell = MeshRadioShell(drive: drive)
        try shell.start()

        let firstIdentity = MeshConnectionIdentity.create()
        let first = try XCTUnwrap(try shell.onLinkUp(firstIdentity, role: .central))
        let duplicate = try XCTUnwrap(try shell.onLinkUp(firstIdentity, role: .central))
        XCTAssertEqual(first.link, duplicate.link)
        XCTAssertEqual(shell.activeLinkCount, 1)
        XCTAssertEqual(node.linkUpInitiators, [true])

        try shell.onLinkDown(first)
        let secondIdentity = MeshConnectionIdentity.create()
        let second = try XCTUnwrap(try shell.onLinkUp(secondIdentity, role: .peripheral))
        XCTAssertGreaterThan(second.link, first.link)
        XCTAssertEqual(node.linkUpInitiators, [true, false])

        // A delayed disconnect for the retired identity cannot tear down the
        // replacement connection because every physical connection owns a
        // fresh identity object.
        try shell.onLinkDown(first)
        XCTAssertEqual(shell.activeLinkCount, 1)
        XCTAssertTrue(shell.isCurrent(second))
    }

    func testRadioShellRejectsDataAfterDisconnect() throws {
        let node = TestNode()
        let shell = MeshRadioShell(drive: makeDrive(node: node))
        try shell.start()
        let identity = MeshConnectionIdentity.create()

        let binding = try XCTUnwrap(try shell.onLinkUp(identity, role: .central))
        try shell.onLinkData(binding, bytes: [4, 5])
        XCTAssertEqual(node.receivedDataCount, 1)
        try shell.onLinkDown(binding)
        try shell.onLinkData(binding, bytes: [6])
        XCTAssertEqual(node.receivedDataCount, 1)
    }

    func testRadioShellRejectsDataFromRetiredBindingAfterReconnect() throws {
        let node = TestNode()
        let shell = MeshRadioShell(drive: makeDrive(node: node))
        try shell.start()

        let first = try XCTUnwrap(
            try shell.onLinkUp(MeshConnectionIdentity.create(), role: .central)
        )
        try shell.onLinkDown(first)
        let second = try XCTUnwrap(
            try shell.onLinkUp(MeshConnectionIdentity.create(), role: .central)
        )

        try shell.onLinkData(first, bytes: [1])
        try shell.onLinkData(second, bytes: [2])

        XCTAssertEqual(node.receivedDataCount, 1)
    }

    func testRadioShellReadyCallbackRetriesBackpressuredFrame() throws {
        let node = TestNode(send: OutboundFrame(link: 7, bytes: Array(UInt8(0)..<UInt8(25))))
        let gatt = TestGatt(acceptedNotifications: [false])
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: gatt,
            foregroundRelayCapable: false
        )
        let shell = MeshRadioShell(drive: drive)
        try shell.start()
        let binding = try XCTUnwrap(
            try shell.onLinkUp(MeshConnectionIdentity.create(), role: .central)
        )
        XCTAssertEqual(gatt.gattNotifications.map { $0.payload.count }, [20])

        try shell.onReadyToSend(binding)
        XCTAssertEqual(gatt.gattNotifications.map { $0.payload.count }, [20, 5])
    }

    func testRadioShellRejectsBindingWhenDriveFailsClosedDuringAdmission() throws {
        let node = TestNode()
        node.tripOverloadOnLinkUp = true
        let shell = MeshRadioShell(drive: makeDrive(node: node))
        try shell.start()

        let binding = try shell.onLinkUp(
            MeshConnectionIdentity.create(),
            role: .central
        )

        XCTAssertNil(binding)
        XCTAssertFalse(shell.isRunning)
        XCTAssertEqual(shell.activeLinkCount, 0)
    }

    func testRadioShellAcceptsOnlyDocumentedRestorationIdentities() {
        let shell = MeshRadioShell(drive: makeDrive(node: TestNode()))

        XCTAssertTrue(shell.onRestored(.central, identifier: MeshRestorationIdentity.central))
        XCTAssertFalse(shell.onRestored(.central, identifier: "mmp.wrong"))
        XCTAssertTrue(shell.onRestored(.peripheral, identifier: MeshRestorationIdentity.peripheral))
        XCTAssertEqual(
            shell.restorationState,
            MeshRestorationState(central: true, peripheral: true)
        )
    }

    func testRadioShellMapsForegroundCapabilityToHonestTier() throws {
        let node = TestNode()
        let shell = MeshRadioShell(drive: makeDrive(node: node))
        try shell.start()

        guard case .leaf? = node.tier else {
            return XCTFail("background capability must select leaf tier")
        }
        shell.setForegroundRelayCapable(true)
        guard case .relay? = node.tier else {
            return XCTFail("foreground capability must select relay tier")
        }
        shell.setForegroundRelayCapable(false)
        guard case .leaf? = node.tier else {
            return XCTFail("capability loss must demote to leaf tier")
        }
    }

    func testRadioShellPreservesMonotonicDriveClock() throws {
        let shell = MeshRadioShell(drive: makeDrive(node: TestNode(), now: 100))
        try shell.start()
        try shell.pumpAt(200)

        XCTAssertThrowsError(try shell.pumpAt(199)) { error in
            XCTAssertEqual(error as? MeshDriveError, .clockMovedBackwards)
        }
    }

    func testBindingRegistryBoundsAdmissionAndFailsBeforeTokenWrap() throws {
        let exhausted = MeshLinkTokenAllocator(firstToken: UInt64.max)
        XCTAssertThrowsError(try exhausted.allocate()) { error in
            XCTAssertEqual(error as? MeshRadioShellError, .linkTokenExhausted)
        }

        let registry = MeshConnectionBindingRegistry(maxBindings: 1)
        let firstIdentity = MeshConnectionIdentity.create()
        let secondIdentity = MeshConnectionIdentity.create()
        XCTAssertNotNil(try registry.open(firstIdentity, role: .central))
        XCTAssertNil(try registry.open(secondIdentity, role: .peripheral))
    }

    func testPublicConsumerCanStartPumpRefreshBeaconAndStop() throws {
        let node = TestNode()
        let clock = TestClock(now: 1_000)
        let advertiser = TestAdvertiser()
        let scanner = TestScanner()
        let gatt = TestGatt()
        let drive = MeshDrive(
            node: node,
            clock: clock,
            advertiser: advertiser,
            scanner: scanner,
            gatt: gatt,
            foregroundRelayCapable: false
        )

        try drive.start()
        XCTAssertTrue(drive.isRunning)
        guard case .leaf? = node.tier else {
            return XCTFail("consumer must be able to observe the selected tier")
        }

        clock.now = 2_000
        try drive.pump()
        try drive.refreshBeacon(nowMs: 2_000)
        XCTAssertEqual(advertiser.serviceData ?? [], [2])

        drive.setForegroundRelayCapable(true)
        guard case .relay? = node.tier else {
            return XCTFail("consumer must be able to switch to relay tier")
        }
        drive.stop()
        XCTAssertFalse(drive.isRunning)
        XCTAssertEqual(advertiser.stopCount, 1)
        XCTAssertEqual(scanner.stopCount, 1)
    }

    private func makeDrive(node: TestNode, now: UInt64 = 1) -> MeshDrive {
        MeshDrive(
            node: node,
            clock: TestClock(now: now),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false
        )
    }

    func testPublicConsumerSeesMtuThreeFragmentationAndBackpressure() throws {
        let node = TestNode(send: OutboundFrame(link: 7, bytes: Array(UInt8(0)..<UInt8(45))))
        let gatt = TestGatt(acceptedNotifications: [false])
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: gatt,
            foregroundRelayCapable: false
        )

        try drive.start()
        try drive.onLinkUp(7, initiator: true)
        XCTAssertEqual(gatt.gattNotifications.count, 1)
        XCTAssertEqual(gatt.gattNotifications[0].payload.count, 20)

        try drive.pump()
        XCTAssertEqual(gatt.gattNotifications.map { $0.payload.count }, [20, 20, 20, 5])
    }

    func testPublicConsumerCanHandleStartupFailureWithoutRemainingRunning() {
        let advertiser = TestAdvertiser(startError: TestError.failed)
        let drive = MeshDrive(
            node: TestNode(),
            clock: TestClock(now: 1),
            advertiser: advertiser,
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false
        )

        XCTAssertThrowsError(try drive.start())
        XCTAssertFalse(drive.isRunning)
        XCTAssertEqual(advertiser.stopCount, 1)
    }
}

private enum TestError: Error {
    case failed
}

private final class TestClock: MonotonicClock {
    var now: UInt64

    init(now: UInt64) {
        self.now = now
    }

    func nowMs() -> UInt64 {
        now
    }
}

private final class TestAdvertiser: Advertiser {
    var serviceData: [UInt8]?
    var stopCount = 0
    private let startError: Error?

    init(startError: Error? = nil) {
        self.startError = startError
    }

    func start(serviceData: [UInt8]) throws {
        if let startError {
            throw startError
        }
        self.serviceData = serviceData
    }

    func stop() throws {
        stopCount += 1
    }
}

private final class TestScanner: Scanner {
    var stopCount = 0

    func start(onResult: @escaping (ScanResult) -> Void) throws {}

    func stop() throws {
        stopCount += 1
    }
}

private final class TestGatt: GattAdapter {
    struct Notification {
        let link: UInt64
        let payload: [UInt8]
    }

    var gattNotifications: [Notification] = []
    private var acceptedNotifications: [Bool]

    init(acceptedNotifications: [Bool] = []) {
        self.acceptedNotifications = acceptedNotifications
    }

    func mtu(link: UInt64) throws -> Int {
        23
    }

    func notifyFragment(link: UInt64, payload: [UInt8]) throws -> Bool {
        gattNotifications.append(Notification(link: link, payload: payload))
        if acceptedNotifications.isEmpty {
            return true
        }
        return acceptedNotifications.removeFirst()
    }

    func close(link: UInt64) throws {}
}

private final class TestNode: MeshNode {
    var tier: MeshTier?
    var linkUpInitiators: [Bool] = []
    var receivedDataCount = 0
    var tripOverloadOnLinkUp = false
    private var eventDrops: UInt64 = 0
    private var nextSend: OutboundFrame?
    private var linkIsUp = false

    init(send: OutboundFrame? = nil) {
        nextSend = send
    }

    func setTier(_ tier: MeshTier) {
        self.tier = tier
    }

    func beacon(nowMs: UInt64) -> [UInt8] {
        [UInt8(nowMs / 1_000)]
    }

    func tick(nowMs: UInt64) {}
    func pollClose() -> UInt64? { nil }
    func closeActionsDropped() -> UInt64 { 0 }
    func eventQueueDropped() -> UInt64 { eventDrops }

    func pollSend() -> OutboundFrame? {
        guard linkIsUp else { return nil }
        defer { nextSend = nil }
        return nextSend
    }

    func pollEvent() -> Any? { nil }
    func onBeacon(serviceData: [UInt8], rssi: Int8) {}
    func linkUp(_ link: UInt64, initiator: Bool) {
        linkIsUp = true
        linkUpInitiators.append(initiator)
        if tripOverloadOnLinkUp { eventDrops = 1 }
    }
    func linkData(_ link: UInt64, bytes: [UInt8]) {
        receivedDataCount += 1
    }
    func linkDown(_ link: UInt64) {
        linkIsUp = false
    }
}
