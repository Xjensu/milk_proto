package org.meshmilk.platform

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * A downstream JVM consumer fixture.  The test intentionally implements the
 * public adapter interfaces outside the coordinator package and supplies no
 * Android classes, proving that the source module is consumable as plain
 * Kotlin/JVM code.
 */
class MeshDriveConsumerTest {
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

    private class FakeNode : MeshNode {
        var tier: MeshTier? = null
        var tickCount: Int = 0

        override fun setTier(tier: MeshTier) {
            this.tier = tier
        }

        override fun beacon(nowMs: Long): ByteArray = byteArrayOf(nowMs.toByte())

        override fun tick(nowMs: Long) {
            tickCount += 1
        }

        override fun pollClose(): Long? = null

        override fun closeActionsDropped(): Long = 0L

        override fun eventQueueDropped(): Long = 0L

        override fun pollSend(): OutboundFrame? = null

        override fun pollEvent(): Any? = null

        override fun onBeacon(serviceData: ByteArray, rssi: Byte) = Unit

        override fun linkUp(link: Long, initiator: Boolean) = Unit

        override fun linkData(link: Long, bytes: ByteArray) = Unit

        override fun linkDown(link: Long) = Unit
    }

    private class RecordingAdvertiser : Advertiser {
        override fun start(serviceData: ByteArray) = Unit

        override fun stop() = Unit
    }

    private class NoopScanner : Scanner {
        override fun start(onResult: (ScanResult) -> Unit) = Unit

        override fun stop() = Unit
    }

    private class NoopGatt : GattAdapter {
        override fun mtu(link: Long): Int = MeshDrive.MIN_ATT_MTU

        override fun notifyFragment(link: Long, payload: ByteArray): Boolean = true

        override fun close(link: Long) = Unit
    }
}
