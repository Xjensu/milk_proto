package org.meshmilk.platform

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotSame
import kotlin.test.assertTrue

/**
 * A downstream JVM consumer fixture.  The test intentionally implements the
 * public adapter interfaces outside the coordinator package and supplies no
 * Android classes, proving that the source module is consumable as plain
 * Kotlin/JVM code.
 */
class MeshDriveConsumerTest {
    @Test
    fun dutyCycleDefaultsMatchMmpBaselineAndAreAppliedOnceBeforeRadioStart() {
        val sink = RecordingDutyCycleSink()
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            dutyCycleSink = sink,
        )

        drive.start()

        assertEquals(listOf(DutyCycleRequest()), sink.requests)
        drive.stop()
    }

    @Test
    fun dutyCycleValidCandidateIsAccepted() {
        val request = DutyCycleRequest(
            baseAdvertisementIntervalMs = 2_000,
            scanWindowMs = 30,
            scanIntervalMs = 2_000,
            burstScanDurationMs = 0,
            burstScanMaxPerMinute = 0,
            connectionIntervalMs = 15,
            supervisionTimeoutMs = 4_000,
        )
        request.validate()
    }

    @Test
    fun dutyCycleSinkReceivesAnIndependentValueSnapshot() {
        val request = DutyCycleRequest(baseAdvertisementIntervalMs = 2_000)
        val sink = RecordingDutyCycleSink()
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            dutyCycleRequest = request,
            dutyCycleSink = sink,
        )

        drive.start()

        assertEquals(request, sink.requests.single())
        assertNotSame(request, sink.requests.single())
        drive.stop()
    }

    @Test
    fun dutyCycleRejectsInvalidJitterWindowAndBurstBeforeAdapterStart() {
        val cases = listOf(
            DutyCycleRequest(advertisementJitterPercent = 9),
            DutyCycleRequest(scanWindowMs = 1_281),
            DutyCycleRequest(scanWindowMs = 100, scanIntervalMs = 2_000),
            DutyCycleRequest(burstScanDurationMs = 30_001),
            DutyCycleRequest(burstScanMaxPerMinute = 7),
            DutyCycleRequest(burstScanDurationMs = 0, burstScanMaxPerMinute = 1),
            DutyCycleRequest(supervisionTimeoutMs = 8_000),
            DutyCycleRequest(perSessionAirtimeCapMs = 59_999),
        )
        cases.forEach { request ->
            val sink = RecordingDutyCycleSink()
            val advertiser = RecordingAdvertiser()
            val drive = MeshDrive(
                node = FakeNode(),
                clock = MonotonicClock { 1_000L },
                advertiser = advertiser,
                scanner = NoopScanner(),
                gatt = NoopGatt(),
                foregroundRelayCapable = false,
                dutyCycleRequest = request,
                dutyCycleSink = sink,
            )

            assertFailsWith<IllegalArgumentException> { drive.start() }
            assertTrue(sink.requests.isEmpty())
            assertEquals(0, advertiser.startCount)
            assertTrue(!drive.isRunning)
        }
    }

    @Test
    fun persistenceFlushIsExplicitAndRemainsAvailableAfterRadioStop() {
        val sink = RecordingPersistenceSink()
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            persistenceSink = sink,
        )

        drive.start()
        drive.stop()
        assertEquals(0, sink.calls)

        drive.flushPersistence()
        assertEquals(1, sink.calls)
    }

    @Test
    fun persistenceFlushFailurePropagatesWithoutChangingRadioLifecycle() {
        val sink = RecordingPersistenceSink(IllegalStateException("flush failed"))
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            persistenceSink = sink,
        )

        drive.start()
        assertFailsWith<IllegalStateException> { drive.flushPersistence() }
        assertTrue(drive.isRunning)
        drive.stop()
    }

    @Test
    fun defaultPersistenceSinkIsAStableNoOp() {
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )

        drive.flushPersistence()
    }

    @Test
    fun secretStatePersistenceIsExplicitAndReturnsOnlyGenerationMetadata() {
        val secret = RecordingSecretStatePersistence(restoreGeneration = null, persistGeneration = 7L)
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            secretStatePersistence = secret,
        )

        // Secret-state operations are explicit and may be called before radio
        // startup (restore) or after teardown (persist).
        assertEquals(null, drive.restoreSecretState())
        drive.start()
        drive.stop()
        assertEquals(7L, drive.persistSecretState())
        assertEquals(1, secret.restoreCalls)
        assertEquals(1, secret.persistCalls)
    }

    @Test
    fun secretStatePersistIsRejectedBeforeFirstSuccessfulRadioCycle() {
        val secret = RecordingSecretStatePersistence(restoreGeneration = null, persistGeneration = 7L)
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            secretStatePersistence = secret,
        )

        assertFailsWith<IllegalStateException> { drive.persistSecretState() }
        assertEquals(0, secret.persistCalls)
    }

    @Test
    fun unavailableSecretStateCapabilityFailsClosedAndIsNeverImplicit() {
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )

        assertFailsWith<IllegalStateException> { drive.restoreSecretState() }
        assertFailsWith<IllegalStateException> { drive.persistSecretState() }
        // Starting and stopping the radio must not silently invoke a missing
        // keystore adapter.
        drive.start()
        drive.stop()
    }

    @Test
    fun secretStateGenerationMustBePositive() {
        val secret = RecordingSecretStatePersistence(restoreGeneration = 0L, persistGeneration = 0L)
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            secretStatePersistence = secret,
        )

        assertFailsWith<IllegalArgumentException> { drive.restoreSecretState() }
        assertFailsWith<IllegalArgumentException> { drive.persistSecretState() }
    }

    @Test
    fun secretStateOperationsAreRejectedWhileRadioIsRunning() {
        val secret = RecordingSecretStatePersistence(restoreGeneration = 3L, persistGeneration = 4L)
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            secretStatePersistence = secret,
        )

        drive.start()
        assertFailsWith<IllegalStateException> { drive.restoreSecretState() }
        assertFailsWith<IllegalStateException> { drive.persistSecretState() }
        assertEquals(0, secret.restoreCalls)
        assertEquals(0, secret.persistCalls)
        drive.stop()
    }

    @Test
    fun publicCoordinatorCanBeStartedAndStoppedByAConsumer() {
        val node = FakeNode()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )

        drive.start()
        assertTrue(drive.isRunning)
        assertEquals(MeshTier.LEAF, node.tier)

        drive.stop()
        assertTrue(!drive.isRunning)
        assertEquals(1, node.tickCount)
    }

    @Test
    fun tokenAndBindingTypesAreUsableByAConsumer() {
        val allocator = LinkTokenAllocator(7L)
        assertEquals(7L, allocator.allocate())

        val registry = ConnectionBindingRegistry(maxBindings = 1, allocator = allocator)
        val identity = ConnectionIdentity.create()
        val binding = registry.open(identity)

        assertTrue(binding != null)
        assertTrue(registry.isCurrent(binding!!))
        assertTrue(registry.close(binding))
        assertEquals(0, registry.size())
    }

    @Test
    fun scannerFilterAndPayloadOwnershipAreEnforced() {
        val node = FakeNode()
        val scanner = RecordingScanner()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = scanner,
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )

        drive.start()
        assertEquals(MmpBleService.SERVICE_UUID, scanner.serviceUuid)

        val ignored = byteArrayOf(1, 2)
        drive.onScanResult(ScanResult(ignored, (-40).toByte(), "00000000-0000-0000-0000-000000000000"))
        drive.onScanResult(ScanResult(byteArrayOf(9), (-40).toByte()))
        assertEquals(0, node.beacons.size)

        val accepted = byteArrayOf(3, 4)
        drive.onScanResult(ScanResult(accepted, (-41).toByte(), MmpBleService.SERVICE_UUID.uppercase()))
        accepted[0] = 99
        assertEquals(1, node.beacons.size)
        assertContentEquals(byteArrayOf(3, 4), node.beacons.single())
    }

    @Test
    fun driveLoopOwnerRejectsCallbacksFromAnotherThread() {
        val drive = MeshDrive(
            node = FakeNode(),
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )
        drive.start()

        var failure: Throwable? = null
        val thread = Thread {
            try {
                drive.pumpAt(1_001L)
            } catch (error: Throwable) {
                failure = error
            }
        }
        thread.start()
        thread.join()

        assertTrue(failure is IllegalStateException)
    }

    @Test
    fun gattBridgeRetiresIdentityBeforeForwardingLateCallbacks() {
        val node = FakeNode()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)
        val identity = ConnectionIdentity.create()
        val binding = bridge.onConnected(identity, initiator = true)

        assertTrue(binding != null)
        assertEquals(1, bridge.activeConnections())
        bridge.onData(binding!!, byteArrayOf(7))
        bridge.onDisconnected(binding)
        bridge.onData(binding, byteArrayOf(8))
        bridge.onDisconnected(binding)

        assertEquals(0, bridge.activeConnections())
        assertEquals(listOf(binding.link), node.linkUps)
        assertEquals(listOf(binding.link), node.linkDowns)
        assertEquals(listOf(binding.link), node.linkDataLinks)
    }

    @Test
    fun gattBridgeRollsBackWhenInitialDrainFails() {
        val node = FakeNode(send = OutboundFrame(11L, byteArrayOf(1)))
        val gatt = FailingGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)
        val identity = ConnectionIdentity.create()

        var failure: Throwable? = null
        try {
            bridge.onConnected(identity, initiator = true)
        } catch (error: Throwable) {
            failure = error
        }

        assertTrue(failure is IllegalStateException)
        assertEquals(0, bridge.activeConnections())
        assertEquals(listOf(11L), node.linkUps)
        assertEquals(listOf(11L), node.linkDowns)
        assertEquals(listOf(1L), gatt.closedLinks)
    }

    @Test
    fun linkUpCallbackFailureRollsBackLogicalAndPhysicalAdmission() {
        val node = FakeNode().also { it.throwOnLinkUp = true }
        val gatt = RecordingGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)

        assertFailsWith<IllegalStateException> {
            bridge.onConnected(ConnectionIdentity.create(), initiator = true)
        }

        assertEquals(0, bridge.activeConnections())
        assertEquals(listOf(1L), gatt.closedLinks)
        assertEquals(listOf(1L), node.linkDowns)
    }

    @Test
    fun duplicateMalformedQualityDoesNotCloseAnAdmittedLink() {
        val node = FakeNode()
        val gatt = RecordingGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)
        val binding = bridge.onConnected(ConnectionIdentity.create(), initiator = true)

        drive.onLinkUp(
            binding!!.link,
            initiator = false,
            quality = LinkQualityHint(mtu = 22, goodputBps = 125_000),
        )

        assertEquals(listOf(binding.link), node.linkUps)
        assertTrue(gatt.closedLinks.isEmpty())
    }

    @Test
    fun gattBridgeForwardsValidQualityAndRejectsInvalidBeforeAdmission() {
        val node = FakeNode()
        val gatt = RecordingGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)
        val quality = LinkQualityHint(mtu = 247, goodputBps = 125_000)
        val binding = bridge.onConnected(ConnectionIdentity.create(), initiator = true, quality = quality)

        assertTrue(binding != null)
        assertEquals(listOf(quality), node.linkUpQualities)
        bridge.onDisconnected(binding!!)

        val rejected = ConnectionIdentity.create()
        assertFailsWith<IllegalArgumentException> {
            bridge.onConnected(
                rejected,
                initiator = false,
                quality = LinkQualityHint(mtu = 22, goodputBps = 125_000),
            )
        }
        assertEquals(0, bridge.activeConnections())
        assertEquals(listOf(binding.link), node.linkUps)
        assertTrue(gatt.closedLinks.size == 1)
    }

    @Test
    fun closeActionIsRetainedWhenNativeTeardownFails() {
        val node = FakeNode()
        val gatt = FailingOnceGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)
        val binding = bridge.onConnected(ConnectionIdentity.create(), initiator = true)
        assertTrue(binding != null)
        node.closeActions.addLast(binding!!.link)

        assertFailsWith<IllegalStateException> { drive.pumpAt(1_000L) }
        assertEquals(listOf(binding.link), gatt.closeAttempts)

        drive.pumpAt(1_000L)
        assertEquals(listOf(binding.link, binding.link), gatt.closeAttempts)
        bridge.onDisconnected(binding)
        drive.stop()
    }

    @Test
    fun admissionRollbackRemovesARequeuedCloseAction() {
        val node = FakeNode().also { it.queueCloseOnLinkUp = true }
        val gatt = FailingOnceGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        drive.start()
        val bridge = GattLinkBridge(drive)

        assertFailsWith<IllegalStateException> {
            bridge.onConnected(ConnectionIdentity.create(), initiator = true)
        }
        // The first close fails during drain; rollback makes one final direct
        // close attempt.  The requeued action must not issue a third attempt.
        assertEquals(2, gatt.closeAttempts.size)
        drive.pumpAt(1_000L)
        assertEquals(2, gatt.closeAttempts.size)
        assertEquals(0, bridge.activeConnections())
        drive.stop()
    }

    @Test
    fun startupRollbackRemovesARequeuedCloseActionBeforeTheNextLifecycle() {
        val node = FakeNode().also { it.queueCloseOnBeaconOnce = true }
        val gatt = FailingOnceGatt()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = gatt,
            foregroundRelayCapable = false,
        )
        var admitted = false
        node.tickAction = {
            if (!admitted) {
                admitted = true
                drive.onLinkUp(17L, initiator = true)
            }
        }

        assertFailsWith<IllegalStateException> { drive.start() }
        // The initial drain fails once, then rollbackStartLinks makes one
        // direct cleanup attempt and retires the logical link.
        assertEquals(2, gatt.closeAttempts.size)
        assertEquals(listOf(17L), node.linkDowns)

        // A new lifecycle must not retry the stale action left by the failed
        // initial drain; the one-shot beacon action is already consumed.
        drive.start()
        assertEquals(2, gatt.closeAttempts.size)
        drive.stop()
    }

    @Test
    fun eventBatchPathPreservesFifoWithAProviderCap() {
        val node = FakeNode(initialEvents = listOf("one", "two", "three"))
        val delivered = mutableListOf<Any>()
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            eventSink = { delivered += it },
        )

        drive.start()

        assertEquals(listOf("one", "two", "three"), delivered)
        assertEquals(listOf(2, 1, 0), node.eventBatchSizes)
        assertTrue(node.eventBatchSizes.all { it <= 2 })
        drive.stop()
    }

    @Test
    fun eventSinkFailureRetainsCurrentEventForTheNextLifecycleAttempt() {
        val node = FakeNode(initialEvents = listOf("event"))
        val delivered = mutableListOf<Any>()
        var failOnce = true
        val drive = MeshDrive(
            node = node,
            clock = MonotonicClock { 1_000L },
            advertiser = RecordingAdvertiser(),
            scanner = NoopScanner(),
            gatt = NoopGatt(),
            foregroundRelayCapable = false,
            eventSink = { event ->
                if (failOnce) {
                    failOnce = false
                    throw IllegalStateException("event bridge unavailable")
                }
                delivered += event
            },
        )

        assertFailsWith<IllegalStateException> { drive.start() }
        assertTrue(!drive.isRunning)

        drive.start()
        assertEquals(listOf("event"), delivered)
        drive.stop()
    }

    private class FakeNode(initialEvents: List<Any> = emptyList()) : MeshNode {
        var tier: MeshTier? = null
        var tickCount: Int = 0
        val beacons: MutableList<ByteArray> = mutableListOf()
        val linkUps: MutableList<Long> = mutableListOf()
        val linkUpQualities: MutableList<LinkQualityHint?> = mutableListOf()
        val linkDowns: MutableList<Long> = mutableListOf()
        val linkDataLinks: MutableList<Long> = mutableListOf()
        val closeActions: java.util.ArrayDeque<Long> = java.util.ArrayDeque()
        private val events: java.util.ArrayDeque<Any> = java.util.ArrayDeque<Any>().also { it.addAll(initialEvents) }
        val eventBatchSizes: MutableList<Int> = mutableListOf()
        var tickAction: (() -> Unit)? = null
        var queueCloseOnBeaconOnce: Boolean = false
        var queueCloseOnLinkUp: Boolean = false
        var throwOnLinkUp: Boolean = false

        override fun setTier(tier: MeshTier) {
            this.tier = tier
        }

        override fun beacon(nowMs: Long): ByteArray {
            if (queueCloseOnBeaconOnce && linkUps.isNotEmpty()) {
                queueCloseOnBeaconOnce = false
                closeActions.addLast(linkUps.last())
            }
            return byteArrayOf(nowMs.toByte())
        }

        override fun tick(nowMs: Long) {
            tickCount += 1
            tickAction?.invoke()
            tickAction = null
        }

        override fun pollClose(): Long? = if (closeActions.isEmpty()) null else closeActions.removeFirst()

        override fun closeActionsDropped(): Long = 0L

        override fun eventQueueDropped(): Long = 0L

        override fun pollSend(): OutboundFrame? = null

        override fun pollEvent(): Any? = if (events.isEmpty()) null else events.removeFirst()

        override fun pollEventBatch(maxItems: Int): List<Any> {
            val batch = ArrayList<Any>(2)
            while (batch.size < minOf(maxItems, 2) && events.isNotEmpty()) {
                batch += events.removeFirst()
            }
            eventBatchSizes += batch.size
            return batch
        }

        override fun onBeacon(serviceData: ByteArray, rssi: Byte) {
            beacons += serviceData
        }

        override fun linkUp(link: Long, initiator: Boolean) {
            linkUps += link
            if (queueCloseOnLinkUp) closeActions.addLast(link)
            if (throwOnLinkUp) throw IllegalStateException("link admission failed")
        }

        override fun linkUp(link: Long, initiator: Boolean, quality: LinkQualityHint?) {
            linkUpQualities += quality
            linkUp(link, initiator)
        }

        override fun linkData(link: Long, bytes: ByteArray) {
            linkDataLinks += link
        }

        override fun linkDown(link: Long) {
            linkDowns += link
        }
    }

    private class RecordingAdvertiser : Advertiser {
        var startCount = 0

        override fun start(serviceData: ByteArray) {
            startCount += 1
        }

        override fun stop() = Unit
    }

    private class RecordingDutyCycleSink : DutyCycleSink {
        val requests = mutableListOf<DutyCycleRequest>()

        override fun applyDutyCycle(request: DutyCycleRequest) {
            requests += request
        }
    }

    private class RecordingPersistenceSink(private val failure: Throwable? = null) : PersistenceSink {
        var calls = 0

        override fun flushPersistence() {
            calls += 1
            failure?.let { throw it }
        }
    }

    private class RecordingSecretStatePersistence(
        private val restoreGeneration: Long?,
        private val persistGeneration: Long,
    ) : SecretStatePersistence {
        var restoreCalls = 0
        var persistCalls = 0

        override fun restoreSecretState(): Long? {
            restoreCalls += 1
            return restoreGeneration
        }

        override fun persistSecretState(): Long {
            persistCalls += 1
            return persistGeneration
        }
    }

    private class NoopScanner : Scanner {
        override fun start(onResult: (ScanResult) -> Unit) = Unit

        override fun stop() = Unit
    }

    private class RecordingScanner : Scanner {
        var serviceUuid: String? = null

        override fun start(onResult: (ScanResult) -> Unit) = Unit

        override fun start(serviceUuid: String, onResult: (ScanResult) -> Unit) {
            this.serviceUuid = serviceUuid
        }

        override fun stop() = Unit
    }

    private class NoopGatt : GattAdapter {
        override fun mtu(link: Long): Int = MeshDrive.MIN_ATT_MTU

        override fun notifyFragment(link: Long, payload: ByteArray): Boolean = true

        override fun close(link: Long) = Unit
    }

    private class FailingGatt : GattAdapter {
        val closedLinks: MutableList<Long> = mutableListOf()

        override fun mtu(link: Long): Int = throw IllegalStateException("MTU unavailable")

        override fun notifyFragment(link: Long, payload: ByteArray): Boolean = true

        override fun close(link: Long) {
            closedLinks += link
        }
    }

    private class RecordingGatt : GattAdapter {
        val closedLinks: MutableList<Long> = mutableListOf()

        override fun mtu(link: Long): Int = MeshDrive.MIN_ATT_MTU

        override fun notifyFragment(link: Long, payload: ByteArray): Boolean = true

        override fun close(link: Long) {
            closedLinks += link
        }
    }

    private class FailingOnceGatt : GattAdapter {
        val closeAttempts: MutableList<Long> = mutableListOf()
        private var failuresRemaining: Int = 1

        override fun mtu(link: Long): Int = MeshDrive.MIN_ATT_MTU

        override fun notifyFragment(link: Long, payload: ByteArray): Boolean = true

        override fun close(link: Long) {
            closeAttempts += link
            if (failuresRemaining > 0) {
                failuresRemaining -= 1
                throw IllegalStateException("native close temporarily unavailable")
            }
        }
    }
}
