import XCTest
import MeshDrive

final class MeshDriveConsumerTests: XCTestCase {
    func testDutyCycleDefaultsMatchMmpBaselineAndApplyOnceBeforeRadioStart() throws {
        let sink = TestDutyCycleSink()
        let advertiser = TestAdvertiser()
        let drive = MeshDrive(
            node: TestNode(),
            clock: TestClock(now: 1),
            advertiser: advertiser,
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            dutyCycleSink: sink
        )

        try drive.start()

        XCTAssertEqual(sink.requests, [MeshDutyCycleRequest()])
        XCTAssertEqual(advertiser.startCount, 1)
        drive.stop()
    }

    func testDutyCycleAcceptsValidCandidate() throws {
        let request = MeshDutyCycleRequest(
            baseAdvertisementIntervalMs: 2_000,
            scanWindowMs: 30,
            scanIntervalMs: 2_000,
            burstScanDurationMs: 0,
            burstScanMaxPerMinute: 0,
            connectionIntervalMs: 15,
            supervisionTimeoutMs: 4_000
        )

        XCTAssertNoThrow(try request.validate())
    }

    func testDutyCycleRejectsInvalidValuesBeforeRadioStart() {
        let requests = [
            MeshDutyCycleRequest(advertisementJitterPercent: 9),
            MeshDutyCycleRequest(scanWindowMs: 1_281),
            MeshDutyCycleRequest(scanWindowMs: 100, scanIntervalMs: 2_000),
            MeshDutyCycleRequest(burstScanDurationMs: 30_001),
            MeshDutyCycleRequest(burstScanMaxPerMinute: 7),
            MeshDutyCycleRequest(burstScanDurationMs: 0, burstScanMaxPerMinute: 1),
            MeshDutyCycleRequest(supervisionTimeoutMs: 8_000),
            MeshDutyCycleRequest(perSessionAirtimeCapMs: 59_999)
        ]

        for request in requests {
            let sink = TestDutyCycleSink()
            let advertiser = TestAdvertiser()
            let drive = MeshDrive(
                node: TestNode(),
                clock: TestClock(now: 1),
                advertiser: advertiser,
                scanner: TestScanner(),
                gatt: TestGatt(),
                foregroundRelayCapable: false,
                dutyCycleRequest: request,
                dutyCycleSink: sink
            )

            XCTAssertThrowsError(try drive.start())
            XCTAssertTrue(sink.requests.isEmpty)
            XCTAssertEqual(advertiser.startCount, 0)
            XCTAssertFalse(drive.isRunning)
        }
    }

    func testPersistenceFlushIsExplicitAndRemainsAvailableAfterRadioStop() throws {
        let sink = TestPersistenceSink()
        let drive = MeshDrive(
            node: TestNode(),
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            persistenceSink: sink
        )
        let shell = MeshRadioShell(drive: drive)

        try shell.start()
        shell.stop()
        XCTAssertEqual(sink.calls, 0)

        try shell.flushPersistence()
        XCTAssertEqual(sink.calls, 1)
    }

    func testPersistenceFlushFailurePropagatesWithoutChangingRadioLifecycle() throws {
        let sink = TestPersistenceSink(error: TestError.persistenceFailed)
        let drive = MeshDrive(
            node: TestNode(),
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            persistenceSink: sink
        )

        try drive.start()
        XCTAssertThrowsError(try drive.flushPersistence()) { error in
            XCTAssertEqual(error as? TestError, .persistenceFailed)
        }
        XCTAssertTrue(drive.isRunning)
        drive.stop()
    }

    func testDefaultPersistenceSinkIsAStableNoOp() throws {
        let drive = makeDrive(node: TestNode())

        XCTAssertNoThrow(try drive.flushPersistence())
    }

    func testSecretStatePersistenceIsExplicitAndReturnsOnlyGenerationMetadata() throws {
        let secret = TestSecretStatePersistence(restoreGeneration: nil, persistGeneration: 7)
        let drive = makeDrive(node: TestNode(), secretStatePersistence: secret)
        let shell = MeshRadioShell(drive: drive)

        XCTAssertNil(try shell.restoreSecretState())
        try shell.start()
        shell.stop()
        XCTAssertEqual(try shell.persistSecretState(), 7)
        XCTAssertEqual(secret.restoreCalls, 1)
        XCTAssertEqual(secret.persistCalls, 1)
    }

    func testSecretStatePersistIsRejectedBeforeFirstSuccessfulRadioCycle() throws {
        let secret = TestSecretStatePersistence(restoreGeneration: nil, persistGeneration: 7)
        let drive = makeDrive(node: TestNode(), secretStatePersistence: secret)

        XCTAssertThrowsError(try drive.persistSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .invalidLifecycle)
        }
        XCTAssertEqual(secret.persistCalls, 0)
    }

    func testUnavailableSecretStateCapabilityFailsClosedAndIsNeverImplicit() throws {
        let drive = makeDrive(node: TestNode())

        XCTAssertThrowsError(try drive.restoreSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .unavailable)
        }
        XCTAssertThrowsError(try drive.persistSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .unavailable)
        }

        // Radio lifecycle must not silently invoke a missing Keychain adapter.
        try drive.start()
        drive.stop()
    }

    func testSecretStateGenerationMustBePositive() throws {
        let secret = TestSecretStatePersistence(restoreGeneration: 0, persistGeneration: 0)
        let drive = makeDrive(node: TestNode(), secretStatePersistence: secret)

        XCTAssertThrowsError(try drive.restoreSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .invalidGeneration)
        }
        XCTAssertThrowsError(try drive.persistSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .invalidGeneration)
        }
    }

    func testSecretStateRestoreIsRejectedAfterRadioStartup() throws {
        let secret = TestSecretStatePersistence(restoreGeneration: 9, persistGeneration: 10)
        let drive = makeDrive(node: TestNode(), secretStatePersistence: secret)

        try drive.start()
        defer { drive.stop() }

        XCTAssertThrowsError(try drive.restoreSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .invalidLifecycle)
        }
        XCTAssertThrowsError(try drive.persistSecretState()) { error in
            XCTAssertEqual(error as? MeshSecretStateError, .invalidLifecycle)
        }
        XCTAssertEqual(secret.restoreCalls, 0)
        XCTAssertEqual(secret.persistCalls, 0)
    }

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

    func testRadioShellForwardsQualityAndRejectsInvalidBeforeAdmission() throws {
        let node = TestNode()
        let gatt = TestGatt()
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
        let quality = MeshLinkQualityHint(mtu: 247, goodputBps: 125_000)
        let binding = try XCTUnwrap(
            try shell.onLinkUp(
                MeshConnectionIdentity.create(),
                role: .central,
                quality: quality
            )
        )
        XCTAssertEqual(node.linkUpQualityHints, [quality])
        try shell.onLinkDown(binding)

        XCTAssertThrowsError(
            try shell.onLinkUp(
                MeshConnectionIdentity.create(),
                role: .peripheral,
                quality: MeshLinkQualityHint(mtu: 22, goodputBps: 125_000)
            )
        ) { error in
            XCTAssertEqual(
                error as? MeshDriveError,
                .invalidLinkQuality(mtu: 22, goodputBps: 125_000)
            )
        }
        XCTAssertEqual(shell.activeLinkCount, 0)
        XCTAssertEqual(node.linkUpInitiators, [true])
        XCTAssertEqual(gatt.closedLinks.count, 1)
    }

    func testRadioShellClosesPhysicalLinkWhenInitialAdmissionDrainFails() throws {
        let node = TestNode(send: OutboundFrame(link: 1, bytes: [7]))
        let gatt = TestGatt(mtuError: TestError.mtuUnavailable)
        let shell = MeshRadioShell(
            drive: MeshDrive(
                node: node,
                clock: TestClock(now: 1),
                advertiser: TestAdvertiser(),
                scanner: TestScanner(),
                gatt: gatt,
                foregroundRelayCapable: false
            )
        )
        try shell.start()

        XCTAssertThrowsError(
            try shell.onLinkUp(MeshConnectionIdentity.create(), role: .central)
        )
        XCTAssertEqual(gatt.closedLinks, [1])
        XCTAssertEqual(shell.activeLinkCount, 0)
        XCTAssertEqual(node.linkUpInitiators, [true])
    }

    func testDuplicateMalformedQualityDoesNotCloseAnAdmittedLink() throws {
        let node = TestNode()
        let gatt = TestGatt()
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: gatt,
            foregroundRelayCapable: false
        )
        try drive.start()
        try drive.onLinkUp(1, initiator: true)

        XCTAssertNoThrow(
            try drive.onLinkUp(
                1,
                initiator: false,
                quality: MeshLinkQualityHint(mtu: 22, goodputBps: 125_000)
            )
        )
        XCTAssertEqual(node.linkUpInitiators, [true])
        XCTAssertTrue(gatt.closedLinks.isEmpty)
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

    func testCloseBatchRetainsActionWhenNativeTeardownFails() throws {
        let node = TestNode()
        let gatt = TestGatt(closeFailuresRemaining: 1)
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
        node.closeActions.append(7)

        XCTAssertThrowsError(try drive.pumpAt(1)) { error in
            XCTAssertEqual(error as? TestError, .closeUnavailable)
        }
        XCTAssertEqual(gatt.closeAttempts, [7])

        try drive.pumpAt(1)
        XCTAssertEqual(gatt.closeAttempts, [7, 7])
        XCTAssertEqual(node.closeBatchSizes, [64, 64, 64, 64])

        try drive.onLinkDown(7)
        drive.stop()
    }

    func testStartupRollbackRetiresRequeuedCloseBeforeNextLifecycle() throws {
        let node = TestNode()
        node.queueCloseOnLinkUp = true
        let gatt = TestGatt(closeFailuresRemaining: 1)
        let scanner = TestScanner()
        var drive: MeshDrive!
        drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: scanner,
            gatt: gatt,
            foregroundRelayCapable: false
        )
        scanner.startAction = {
            try drive.onLinkUp(17, initiator: true)
        }

        XCTAssertThrowsError(try drive.start()) { error in
            XCTAssertEqual(error as? TestError, .closeUnavailable)
        }
        XCTAssertFalse(drive.isRunning)
        XCTAssertEqual(gatt.closeAttempts, [17, 17])
        XCTAssertEqual(node.linkDowns, [17])

        try drive.start()
        XCTAssertEqual(gatt.closeAttempts, [17, 17])
        drive.stop()
    }

    func testEventBatchPathPreservesFifoWithProviderCap() throws {
        let node = TestNode(initialEvents: ["one", "two", "three"])
        var delivered: [Any] = []
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            eventSink: { delivered.append($0) }
        )

        try drive.start()

        XCTAssertEqual(delivered.compactMap { $0 as? String }, ["one", "two", "three"])
        XCTAssertEqual(node.eventBatchSizes, [2, 1, 0])
        XCTAssertTrue(node.eventBatchSizes.allSatisfy { $0 <= 64 })
        drive.stop()
    }

    func testEventSinkFailureRetainsCurrentEventForNextLifecycleAttempt() throws {
        let node = TestNode(initialEvents: ["event"])
        var delivered: [Any] = []
        var failOnce = true
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            eventSink: { event in
                if failOnce {
                    failOnce = false
                    throw TestError.eventSinkUnavailable
                }
                delivered.append(event)
            }
        )

        XCTAssertThrowsError(try drive.start()) { error in
            XCTAssertEqual(error as? TestError, .eventSinkUnavailable)
        }
        XCTAssertFalse(drive.isRunning)

        try drive.start()
        XCTAssertEqual(delivered.compactMap { $0 as? String }, ["event"])
        drive.stop()
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

    func testScanResultRequiresExactMmpServiceUuid() throws {
        let node = TestNode()
        let drive = makeDrive(node: node)

        // A callback before startup remains ignored.
        drive.onScanResult(
            ScanResult(serviceData: [9], rssi: -40, serviceUuid: MmpBleService.serviceUUID)
        )
        XCTAssertEqual(node.receivedBeaconCount, 0)

        try drive.start()
        drive.onScanResult(
            ScanResult(serviceData: [1], rssi: -41, serviceUuid: MmpBleService.serviceUUID.uppercased())
        )
        drive.onScanResult(ScanResult(serviceData: [2], rssi: -42, serviceUuid: "wrong"))
        drive.onScanResult(ScanResult(serviceData: [3], rssi: -43))

        XCTAssertEqual(node.receivedBeaconCount, 1)
        XCTAssertEqual(node.receivedBeacons, [[1]])
        drive.stop()
    }

    func testScanResultEmptyPayloadAndStoppedDriveRemainIgnored() throws {
        let node = TestNode()
        let drive = makeDrive(node: node)

        try drive.start()
        drive.onScanResult(
            ScanResult(serviceData: [], rssi: -40, serviceUuid: MmpBleService.serviceUUID)
        )
        XCTAssertEqual(node.receivedBeaconCount, 0)

        drive.stop()
        drive.onScanResult(
            ScanResult(serviceData: [4], rssi: -40, serviceUuid: MmpBleService.serviceUUID)
        )
        XCTAssertEqual(node.receivedBeaconCount, 0)
    }

    private func makeDrive(
        node: TestNode,
        now: UInt64 = 1,
        secretStatePersistence: SecretStatePersistence = UnavailableSecretStatePersistence()
    ) -> MeshDrive {
        MeshDrive(
            node: node,
            clock: TestClock(now: now),
            advertiser: TestAdvertiser(),
            scanner: TestScanner(),
            gatt: TestGatt(),
            foregroundRelayCapable: false,
            secretStatePersistence: secretStatePersistence
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

    func testStartupOverloadReturnsAnErrorAndRollsBackRadioResources() {
        let node = TestNode()
        node.tripOverloadOnStart = true
        let advertiser = TestAdvertiser()
        let scanner = TestScanner()
        let drive = MeshDrive(
            node: node,
            clock: TestClock(now: 1),
            advertiser: advertiser,
            scanner: scanner,
            gatt: TestGatt(),
            foregroundRelayCapable: false
        )

        XCTAssertThrowsError(try drive.start()) { error in
            XCTAssertEqual(error as? MeshDriveError, .hostOverload)
        }
        XCTAssertFalse(drive.isRunning)
        XCTAssertEqual(advertiser.stopCount, 1)
        XCTAssertEqual(scanner.stopCount, 1)
    }
}

private enum TestError: Error, Equatable {
    case failed
    case persistenceFailed
    case mtuUnavailable
    case closeUnavailable
    case eventSinkUnavailable
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
    var startCount = 0
    var stopCount = 0
    private let startError: Error?

    init(startError: Error? = nil) {
        self.startError = startError
    }

    func start(serviceData: [UInt8]) throws {
        if let startError {
            throw startError
        }
        startCount += 1
        self.serviceData = serviceData
    }

    func stop() throws {
        stopCount += 1
    }
}

private final class TestDutyCycleSink: DutyCycleSink {
    var requests: [MeshDutyCycleRequest] = []

    func applyDutyCycle(_ request: MeshDutyCycleRequest) throws {
        requests.append(request)
    }
}

private final class TestPersistenceSink: PersistenceSink {
    var calls = 0
    private let error: Error?

    init(error: Error? = nil) {
        self.error = error
    }

    func flushPersistence() throws {
        calls += 1
        if let error {
            throw error
        }
    }
}

private final class TestScanner: Scanner {
    var stopCount = 0
    var startAction: (() throws -> Void)?

    func start(onResult: @escaping (ScanResult) -> Void) throws {
        if let startAction {
            self.startAction = nil
            try startAction()
        }
    }

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
    var closedLinks: [UInt64] = []
    private var acceptedNotifications: [Bool]
    private let mtuError: Error?
    private var closeFailuresRemaining: Int
    var closeAttempts: [UInt64] = []

    init(
        acceptedNotifications: [Bool] = [],
        mtuError: Error? = nil,
        closeFailuresRemaining: Int = 0
    ) {
        self.acceptedNotifications = acceptedNotifications
        self.mtuError = mtuError
        self.closeFailuresRemaining = closeFailuresRemaining
    }

    func mtu(link: UInt64) throws -> Int {
        if let mtuError {
            throw mtuError
        }
        23
    }

    func notifyFragment(link: UInt64, payload: [UInt8]) throws -> Bool {
        gattNotifications.append(Notification(link: link, payload: payload))
        if acceptedNotifications.isEmpty {
            return true
        }
        return acceptedNotifications.removeFirst()
    }

    func close(link: UInt64) throws {
        closeAttempts.append(link)
        if closeFailuresRemaining > 0 {
            closeFailuresRemaining -= 1
            throw TestError.closeUnavailable
        }
        closedLinks.append(link)
    }
}

private final class TestSecretStatePersistence: SecretStatePersistence {
    let restoreGeneration: UInt64?
    let persistGeneration: UInt64
    var restoreCalls = 0
    var persistCalls = 0

    init(restoreGeneration: UInt64?, persistGeneration: UInt64) {
        self.restoreGeneration = restoreGeneration
        self.persistGeneration = persistGeneration
    }

    func restoreSecretState() throws -> UInt64? {
        restoreCalls += 1
        return restoreGeneration
    }

    func persistSecretState() throws -> UInt64 {
        persistCalls += 1
        return persistGeneration
    }
}

private final class TestNode: MeshNode {
    var tier: MeshTier?
    var linkUpInitiators: [Bool] = []
    var linkUpQualityHints: [MeshLinkQualityHint?] = []
    var receivedBeacons: [[UInt8]] = []
    var receivedDataCount = 0
    var linkDowns: [UInt64] = []
    var tripOverloadOnLinkUp = false
    var tripOverloadOnStart = false
    var queueCloseOnLinkUp = false
    var closeActions: [UInt64] = []
    var closeBatchSizes: [Int] = []
    var eventBatchSizes: [Int] = []
    private var eventDrops: UInt64 = 0
    private var nextSend: OutboundFrame?
    private var linkIsUp = false
    private var events: [Any]

    init(send: OutboundFrame? = nil, initialEvents: [Any] = []) {
        nextSend = send
        events = initialEvents
    }

    func setTier(_ tier: MeshTier) {
        self.tier = tier
    }

    func beacon(nowMs: UInt64) -> [UInt8] {
        [UInt8(nowMs / 1_000)]
    }

    func tick(nowMs: UInt64) {
        if tripOverloadOnStart {
            eventDrops = 1
        }
    }
    func pollClose() -> UInt64? { nil }
    func pollCloseBatch(_ maxItems: Int) -> [UInt64] {
        closeBatchSizes.append(maxItems)
        let count = min(maxItems, closeActions.count)
        guard count > 0 else { return [] }
        let batch = Array(closeActions.prefix(count))
        closeActions.removeFirst(count)
        return batch
    }
    func closeActionsDropped() -> UInt64 { 0 }
    func eventQueueDropped() -> UInt64 { eventDrops }

    func pollSend() -> OutboundFrame? {
        guard linkIsUp else { return nil }
        defer { nextSend = nil }
        return nextSend
    }

    func pollEvent() -> Any? {
        guard !events.isEmpty else { return nil }
        return events.removeFirst()
    }
    func pollEventBatch(_ maxItems: Int) -> [Any] {
        let count = min(maxItems, 2, events.count)
        eventBatchSizes.append(count)
        guard count > 0 else { return [] }
        let batch = Array(events.prefix(count))
        events.removeFirst(count)
        return batch
    }
    func onBeacon(serviceData: [UInt8], rssi: Int8) {
        receivedBeacons.append(serviceData)
    }
    var receivedBeaconCount: Int { receivedBeacons.count }
    func linkUp(_ link: UInt64, initiator: Bool) {
        linkIsUp = true
        linkUpInitiators.append(initiator)
        if queueCloseOnLinkUp {
            queueCloseOnLinkUp = false
            closeActions.append(link)
        }
        if tripOverloadOnLinkUp { eventDrops = 1 }
    }
    func linkUp(_ link: UInt64, initiator: Bool, quality: MeshLinkQualityHint?) {
        linkUpQualityHints.append(quality)
        linkUp(link, initiator: initiator)
    }
    func linkData(_ link: UInt64, bytes: [UInt8]) {
        receivedDataCount += 1
    }
    func linkDown(_ link: UInt64) {
        linkIsUp = false
        linkDowns.append(link)
    }
}
