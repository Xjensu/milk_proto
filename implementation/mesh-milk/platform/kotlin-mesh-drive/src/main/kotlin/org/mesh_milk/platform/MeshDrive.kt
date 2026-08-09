package org.meshmilk.platform

/**
 * Platform-neutral coordinator for the Kotlin shell described by
 * MOBILE_INTEGRATION.md §4.
 *
 * This file intentionally has no Android/CoreBluetooth imports. The Android
 * foreground service supplies the adapters and calls [pump] from its own
 * lifecycle/timer. The coordinator owns ordering and safety rules; it never
 * starts a thread or sleeps.
 */
public enum class MeshTier {
    LEAF,
    RELAY,
}
public data class ScanResult(
    val serviceData: ByteArray?,
    val rssi: Byte,
)

public data class OutboundFrame(
    val link: Long,
    val bytes: ByteArray,
)

/** Minimal node/FFI surface needed by the drive loop. */
public interface MeshNode {
    fun setTier(tier: MeshTier)
    fun beacon(nowMs: Long): ByteArray
    fun tick(nowMs: Long)
    /** Drain one transport-local request to terminate a physical BLE link. */
    fun pollClose(): Long?
    /** Sticky close-queue saturation diagnostic; non-zero is fail-closed. */
    fun closeActionsDropped(): Long
    /** Sticky application-event queue saturation diagnostic; non-zero is fail-closed. */
    fun eventQueueDropped(): Long
    fun pollSend(): OutboundFrame?
    fun pollEvent(): Any?
    fun onBeacon(serviceData: ByteArray, rssi: Byte)
    fun linkUp(link: Long, initiator: Boolean)
    fun linkData(link: Long, bytes: ByteArray)
    fun linkDown(link: Long)
}

/** Monotonic elapsed-realtime source (SystemClock.elapsedRealtime on Android). */
public fun interface MonotonicClock {
    fun nowMs(): Long
}

public interface Advertiser {
    fun start(serviceData: ByteArray)
    fun stop()
}

public interface Scanner {
    fun start(onResult: (ScanResult) -> Unit)
    fun stop()
}

/**
 * Native GATT bridge. [notifyFragment] must return false when the platform
 * cannot accept a notification yet; the coordinator retains that frame and
 * retries it on the next pump.
 */
public interface GattAdapter {
    fun mtu(link: Long): Int
    fun notifyFragment(link: Long, payload: ByteArray): Boolean

    /** Request native teardown; the platform must call [MeshDrive.onLinkDown] after it completes. */
    fun close(link: Long)
}

/**
 * Host-side Kotlin drive loop. All public callbacks are expected on the
 * service's single drive thread/queue.
 */
public class MeshDrive(
    private val node: MeshNode,
    private val clock: MonotonicClock,
    private val advertiser: Advertiser,
    private val scanner: Scanner,
    private val gatt: GattAdapter,
    foregroundRelayCapable: Boolean,
    private val eventSink: (Any) -> Unit = {},
) {
    public companion object {
        /** ATT protocol overhead: negotiated MTU minus these bytes is usable. */
        public const val ATT_HEADER_BYTES: Int = 3
        /** Bluetooth LE ATT minimum MTU; smaller values are invalid input. */
        public const val MIN_ATT_MTU: Int = 23
    }

    private data class PendingFrame(
        val link: Long,
        val bytes: ByteArray,
        // MTU lookup is a platform callback and may fail transiently while
        // the link is still negotiating.  Retain the frame before asking for
        // the MTU so a retry cannot lose a protocol envelope.
        var fragmentSize: Int? = null,
        var offset: Int = 0,
    )

    private var running: Boolean = false
    private var relayCapable: Boolean = foregroundRelayCapable
    private var lastTickMs: Long? = null
    private var advertisedBeacon: ByteArray? = null
    private val activeLinks = LinkedHashSet<Long>()
    private val pending = java.util.ArrayDeque<PendingFrame>()

    public val isRunning: Boolean
        get() = running

    public val lastTick: Long?
        get() = lastTickMs

    /** Sticky transport-close saturation counter exposed for fail-closed hosts. */
    public val closeActionsDropped: Long
        get() = node.closeActionsDropped()

    /** Sticky application-event saturation counter exposed for fail-closed hosts. */
    public val eventQueueDropped: Long
        get() = node.eventQueueDropped()

    public fun start() {
        check(!running) { "MeshDrive is already running" }
        running = true
        var advertiserStarted = false
        var scannerStarted = false
        try {
            applyTier()

            val now = clock.nowMs()
            advance(now, emit = false)
            try {
                val beacon = node.beacon(now)
                // Do not let a native adapter retain or mutate the coordinator's
                // cached payload; ownership crosses the platform boundary here.
                advertiser.start(beacon.copyOf())
                advertisedBeacon = beacon.copyOf()
                advertiserStarted = true
            } catch (error: Throwable) {
                // A platform may have allocated radio state before reporting a
                // startup failure. Stop is the only injected rollback seam;
                // ignore a cleanup failure so the original error remains
                // visible to the host.
                runCatching { advertiser.stop() }
                throw error
            }
            try {
                scanner.start(::onScanResult)
                scannerStarted = true
            } catch (error: Throwable) {
                runCatching { scanner.stop() }
                throw error
            }

            // Startup remains pull-based: only this drain calls pollSend/pollEvent.
            drain()
        } catch (error: Throwable) {
            if (scannerStarted) runCatching { scanner.stop() }
            if (advertiserStarted) runCatching { advertiser.stop() }
            running = false
            throw error
        }
    }

    public fun stop() {
        if (!running) return
        // LinkDown releases the protocol-side session.  The physical GATT
        // connection is a separate host resource and must be torn down too;
        // clear ownership before requesting it so a synchronous adapter
        // callback cannot feed a duplicate LinkDown back into the node.
        val links = activeLinks.toList()
        links.forEach { node.linkDown(it) }
        activeLinks.clear()
        links.forEach { gatt.close(it) }
        pending.clear()
        scanner.stop()
        advertiser.stop()
        advertisedBeacon = null
        running = false
    }

    /**
     * Advance the node using the injected monotonic time and drain output.
     * Equal timestamps are allowed; backwards time is rejected before the node
     * sees it.
     */
    public fun pump() {
        requireRunning()
        advance(clock.nowMs(), emit = true)
    }

    /** Deterministic equivalent of [pump] for host/JVM tests. */
    public fun pumpAt(nowMs: Long) {
        requireRunning()
        advance(nowMs, emit = true)
    }

    /**
     * Publish the current rotating beacon payload again.
     *
     * `MmpNode.beacon()` derives the ephemeral identifier from the supplied
     * monotonic time (the protocol rotates it every beacon slot).  Starting
     * an advertiser once at service creation therefore leaves a long-lived
     * process advertising a stale identifier.  Native adapters should treat
     * [Advertiser.start] as an idempotent replace/update operation; this
     * method gives a host an explicit hook to call at each platform beacon
     * refresh or wakeup without advancing protocol timers a second time.
     *
     * The same monotonic ordering guard as [pump] is applied before the
     * payload is generated.  The method deliberately does not call
     * `MeshNode.tick`: hosts may refresh an advertisement between drive-loop
     * ticks, while all protocol state changes remain owned by [pump].
     */
    public fun refreshBeacon(nowMs: Long) {
        requireRunning()
        val previous = lastTickMs
        require(nowMs >= (previous ?: Long.MIN_VALUE)) {
            "monotonic clock moved backwards: $nowMs < $previous"
        }
        val beacon = node.beacon(nowMs)
        if (advertisedBeacon?.contentEquals(beacon) == true) return
        // The adapter may retain the array while the platform advertiser is
        // active; give it a private copy so the cache remains authoritative.
        advertiser.start(beacon.copyOf())
        advertisedBeacon = beacon.copyOf()
    }

    /**
     * Change the foreground-service capability. Losing that capability always
     * demotes the node to LEAF; the coordinator never silently keeps relay
     * behavior in a background process.
     */
    public fun setForegroundRelayCapable(capable: Boolean) {
        relayCapable = capable
        applyTier()
    }

    public fun onScanResult(result: ScanResult) {
        if (!running) return
        val payload = result.serviceData ?: return
        if (payload.isEmpty()) return
        node.onBeacon(payload.copyOf(), result.rssi)
    }

    public fun onLinkUp(link: Long, initiator: Boolean) {
        requireRunning()
        check(activeLinks.add(link)) { "link is already up: $link" }
        node.linkUp(link, initiator)
        drain()
    }

    public fun onLinkData(link: Long, bytes: ByteArray) {
        requireRunning()
        if (link !in activeLinks) return
        if (bytes.isEmpty()) return
        node.linkData(link, bytes.copyOf())
        drain()
    }

    public fun onLinkDown(link: Long) {
        if (!running || !activeLinks.remove(link)) return
        pending.removeIf { it.link == link }
        node.linkDown(link)
        drain()
    }

    private fun advance(nowMs: Long, emit: Boolean) {
        val previous = lastTickMs
        require(nowMs >= (previous ?: Long.MIN_VALUE)) {
            "monotonic clock moved backwards: $nowMs < $previous"
        }
        node.tick(nowMs)
        lastTickMs = nowMs
        if (emit) drain()
    }

    private fun drain() {
        drainClose()
        drainSend()
        drainEvents()
    }

    /**
     * Close actions are transport-local, not wire frames.  Drop queued output
     * for a link before asking the platform to tear it down; the platform must
     * report the physical completion through [onLinkDown].
     */
    private fun drainClose() {
        while (true) {
            val link = node.pollClose() ?: return
            pending.removeIf { it.link == link }
            gatt.close(link)
        }
    }

    private fun drainSend() {
        while (true) {
            if (pending.isEmpty()) {
                val next = node.pollSend() ?: return
                val pendingFrame = PendingFrame(
                    link = next.link,
                    bytes = next.bytes.copyOf(),
                )
                if (pendingFrame.bytes.isEmpty()) continue
                pending.addLast(pendingFrame)
            }
            val frame = pending.peekFirst() ?: return

            if (frame.fragmentSize == null) {
                // Keep the frame at the head of the queue while the adapter
                // reports a transient MTU error.  A later pump can retry the
                // lookup without asking the Rust node for the envelope again.
                val mtu = gatt.mtu(frame.link)
                require(mtu >= MIN_ATT_MTU) { "invalid ATT MTU $mtu for link ${frame.link}" }
                frame.fragmentSize = mtu - ATT_HEADER_BYTES
            }
            val fragmentSize = frame.fragmentSize ?: return

            val end = minOf(frame.offset + fragmentSize, frame.bytes.size)
            val accepted = gatt.notifyFragment(
                frame.link,
                frame.bytes.copyOfRange(frame.offset, end),
            )
            if (!accepted) return
            frame.offset = end
            if (frame.offset == frame.bytes.size) {
                pending.removeFirst()
            }
        }
    }

    private fun drainEvents() {
        while (true) {
            val event = node.pollEvent() ?: return
            eventSink(event)
        }
    }

    private fun applyTier() {
        node.setTier(if (relayCapable) MeshTier.RELAY else MeshTier.LEAF)
    }

    private fun requireRunning() {
        check(running) { "MeshDrive is not running" }
    }
}
