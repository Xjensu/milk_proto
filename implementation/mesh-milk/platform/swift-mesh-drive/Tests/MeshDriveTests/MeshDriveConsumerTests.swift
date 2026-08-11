import XCTest
import MeshDrive

final class MeshDriveConsumerTests: XCTestCase {
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
    func eventQueueDropped() -> UInt64 { 0 }

    func pollSend() -> OutboundFrame? {
        guard linkIsUp else { return nil }
        defer { nextSend = nil }
        return nextSend
    }

    func pollEvent() -> Any? { nil }
    func onBeacon(serviceData: [UInt8], rssi: Int8) {}
    func linkUp(_ link: UInt64, initiator: Bool) {
        linkIsUp = true
    }
    func linkData(_ link: UInt64, bytes: [UInt8]) {}
    func linkDown(_ link: UInt64) {
        linkIsUp = false
    }
}
