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

/**
 * Wire-level UUIDs used by the native BLE adapter.  The Android shell owns
 * UUID registration and discovery; keeping the value here only makes the
 * SDK-free callback contract explicit and avoids accepting unrelated service
 * payloads by accident.
 */
public object MmpBleService {
    public const val SERVICE_UUID: String = "6d6d7031-0000-4d4d-5031-6d6573686d6c"
}

/**
 * Single-owner guard for a platform drive queue.  Android callbacks must be
 * marshalled onto one Handler/Looper before they reach [MeshDrive].
 *
 * The default implementation binds the first caller and rejects callbacks
 * from another thread.  A consuming host may inject a queue-specific guard
 * when its event-loop identity is not represented by a JVM thread.
 */
public fun interface DriveLoopOwner {
    public fun check()
}

public class ThreadConfinedDriveLoopOwner : DriveLoopOwner {
    private var owner: Thread? = null

    @Synchronized
    override fun check() {
        val current = Thread.currentThread()
        val bound = owner
        if (bound == null) {
            owner = current
        } else {
            check(bound === current) {
                "MeshDrive callbacks must use one serialized drive-loop owner"
            }
        }
    }
}

public data class ScanResult(
    val serviceData: ByteArray?,
    val rssi: Byte,
    /** Native callback must provide the service UUID it actually observed. */
    val serviceUuid: String? = null,
)

public data class OutboundFrame(
    val link: Long,
    val bytes: ByteArray,
)

/**
 * Local transport measurements used by the Rust scheduler.  This record is
 * never serialized into an MMP/1 frame.  Values are validated at admission so
 * a malformed native callback cannot create an unbounded scheduling hint.
 */
public data class LinkQualityHint(
    val mtu: Int,
    val goodputBps: Int,
) {
    public fun isValid(): Boolean =
        mtu in MIN_MTU..MAX_MTU && goodputBps in MIN_GOODPUT_BPS..MAX_GOODPUT_BPS

    public companion object {
        public const val MIN_MTU: Int = 23
        public const val MAX_MTU: Int = 4096
        public const val MIN_GOODPUT_BPS: Int = 1
        public const val MAX_GOODPUT_BPS: Int = 10_000_000
    }
}

/**
 * Host-only radio scheduling request.  These values tune the native BLE
 * adapter; they are never encoded into an MMP/1 frame and do not start a
 * timer in this SDK-free coordinator.  The session-airtime field is forwarded
 * as a host conformance bound; the native adapter/core remains responsible for
 * enforcing it.
 */
public data class DutyCycleRequest(
    val baseAdvertisementIntervalMs: Int = 5_000,
    val advertisementJitterPercent: Int = 10,
    val scanWindowMs: Int = 30,
    val scanIntervalMs: Int = 1_280,
    val burstScanDurationMs: Int = 30_000,
    val burstScanMaxPerMinute: Int = 1,
    val connectionIntervalMs: Int = 30,
    val supervisionTimeoutMs: Int = 4_000,
    val perSessionAirtimeCapMs: Int = 60_000,
) {
    public fun validate() {
        require(baseAdvertisementIntervalMs in MIN_ADVERTISEMENT_INTERVAL_MS..MAX_ADVERTISEMENT_INTERVAL_MS) {
            "baseAdvertisementIntervalMs must be 2000..10000"
        }
        require(advertisementJitterPercent == CONFORMING_JITTER_PERCENT) {
            "advertisementJitterPercent must be exactly 10 for MMP/1"
        }
        require(scanIntervalMs in MIN_SCAN_INTERVAL_MS..MAX_SCAN_INTERVAL_MS) {
            "scanIntervalMs must be 30..60000"
        }
        require(scanWindowMs in MIN_SCAN_WINDOW_MS..scanIntervalMs) {
            "scanWindowMs must be 30..scanIntervalMs"
        }
        require(scanWindowMs.toLong() * MAX_SCAN_DUTY_DENOMINATOR <= scanIntervalMs.toLong()) {
            "scanWindowMs must be at most 2.5% of scanIntervalMs"
        }
        require(burstScanDurationMs in 0..MAX_BURST_SCAN_DURATION_MS) {
            "burstScanDurationMs must be 0..30000"
        }
        require(burstScanMaxPerMinute in 0..MAX_BURST_SCAN_PER_MINUTE) {
            "burstScanMaxPerMinute must be 0..6"
        }
        require((burstScanDurationMs == 0) == (burstScanMaxPerMinute == 0)) {
            "burstScanDurationMs and burstScanMaxPerMinute must both be zero or non-zero"
        }
        require(connectionIntervalMs in MIN_CONNECTION_INTERVAL_MS..MAX_CONNECTION_INTERVAL_MS) {
            "connectionIntervalMs must be 15..30"
        }
        require(supervisionTimeoutMs == CONFORMING_SUPERVISION_TIMEOUT_MS) {
            "supervisionTimeoutMs must be exactly 4000"
        }
        require(perSessionAirtimeCapMs == CONFORMING_SESSION_AIRTIME_CAP_MS) {
            "perSessionAirtimeCapMs must be exactly 60000"
        }
    }

    public companion object {
        public const val MIN_ADVERTISEMENT_INTERVAL_MS: Int = 2_000
        public const val MAX_ADVERTISEMENT_INTERVAL_MS: Int = 10_000
        public const val CONFORMING_JITTER_PERCENT: Int = 10
        public const val MIN_SCAN_WINDOW_MS: Int = 30
        public const val MIN_SCAN_INTERVAL_MS: Int = 30
        public const val MAX_SCAN_INTERVAL_MS: Int = 60_000
        public const val MAX_BURST_SCAN_DURATION_MS: Int = 30_000
        public const val MAX_BURST_SCAN_PER_MINUTE: Int = 6
        /** 1/40 is the exact integer representation of the 2.5% baseline cap. */
        public const val MAX_SCAN_DUTY_DENOMINATOR: Long = 40
        public const val MIN_CONNECTION_INTERVAL_MS: Int = 15
        public const val MAX_CONNECTION_INTERVAL_MS: Int = 30
        public const val CONFORMING_SUPERVISION_TIMEOUT_MS: Int = 4_000
        public const val CONFORMING_SESSION_AIRTIME_CAP_MS: Int = 60_000
    }
}

/** Adapter-facing sink for transport-local scheduling configuration. */
public fun interface DutyCycleSink {
    public fun applyDutyCycle(request: DutyCycleRequest)
}

/**
 * Host-owned persistence boundary for the existing mmp-ffi snapshot store.
 *
 * The coordinator never invokes this callback automatically.  A native host
 * should wrap its `MmpNode.flush()` call and invoke [MeshDrive.flushPersistence]
 * from an explicit background/termination lifecycle hook.  Exceptions are
 * deliberately propagated so the host can record a durability failure; this
 * interface never receives keys or snapshot bytes.
 */
public fun interface PersistenceSink {
    public fun flushPersistence()
}

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
    /**
     * Optional measured transport quality.  The default delegates to the
     * legacy callback so existing FFI adapters remain source-compatible.
     */
    fun linkUp(link: Long, initiator: Boolean, quality: LinkQualityHint?) {
        linkUp(link, initiator)
    }
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

    /**
     * Start a scan filtered by the protocol service UUID.  The default keeps
     * existing SDK-free consumers source-compatible; a native Android
     * adapter should override it and install a controller-side ScanFilter.
     */
    public fun start(serviceUuid: String, onResult: (ScanResult) -> Unit) {
        start(onResult)
    }

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
    private val driveLoopOwner: DriveLoopOwner = ThreadConfinedDriveLoopOwner(),
    private val dutyCycleRequest: DutyCycleRequest = DutyCycleRequest(),
    private val dutyCycleSink: DutyCycleSink = DutyCycleSink { },
    private val persistenceSink: PersistenceSink = PersistenceSink { },
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
    // A physical link remains in this set until the native disconnect
    // callback arrives.  Bound it independently of the Rust session cap:
    // denied LinkUp attempts can otherwise accumulate here when a broken
    // adapter never reports teardown.
    private val maxTrackedLinks: Int = 64
    private val closeRequested = LinkedHashSet<Long>()
    private val pending = java.util.ArrayDeque<PendingFrame>()
    private var overloadLatched: Boolean = false
    private var observedCloseDrops: Long = 0
    private var observedEventDrops: Long = 0

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

    /** True after a sticky transport/event overflow forced fail-closed mode. */
    public val isOverloaded: Boolean
        get() = overloadLatched

    public fun start() {
        driveLoopOwner.check()
        check(!running) { "MeshDrive is already running" }
        check(!overloadLatched) { "MeshDrive is latched after host overload; recreate the node" }
        dutyCycleRequest.validate()
        // Apply exactly once per successful startup, before any native radio
        // adapter is started. The data class is immutable and copy() makes
        // ownership across the host boundary explicit.
        dutyCycleSink.applyDutyCycle(dutyCycleRequest.copy())
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
                scanner.start(MmpBleService.SERVICE_UUID, ::onScanResult)
                scannerStarted = true
            } catch (error: Throwable) {
                runCatching { scanner.stop() }
                throw error
            }

            // Startup remains pull-based: only this drain calls pollSend/pollEvent.
            drain()
            check(running) { "MeshDrive failed closed during startup" }
        } catch (error: Throwable) {
            if (scannerStarted) runCatching { scanner.stop() }
            if (advertiserStarted) runCatching { advertiser.stop() }
            rollbackStartLinks()
            running = false
            throw error
        }
    }

    public fun stop() {
        driveLoopOwner.check()
        if (!running) return
        // LinkDown releases the protocol-side session.  The physical GATT
        // connection is a separate host resource and must be torn down too;
        // clear ownership before requesting it so a synchronous adapter
        // callback cannot feed a duplicate LinkDown back into the node.
        val links = activeLinks.toList()
        links.forEach { node.linkDown(it) }
        activeLinks.clear()
        // A native close can fail after partially touching the controller.
        // Contain each failure so one broken link cannot prevent teardown of
        // the remaining links or leave discovery running indefinitely.
        links.forEach { runCatching { requestPhysicalClose(it) } }
        closeRequested.clear()
        pending.clear()
        // Stop is best-effort cleanup; always finish the coordinator state
        // transition even when a platform adapter reports a teardown error.
        runCatching { scanner.stop() }
        runCatching { advertiser.stop() }
        advertisedBeacon = null
        running = false
    }

    /**
     * Flush host-owned durable state at an explicit lifecycle boundary.
     *
     * This method is valid while the radio is running or stopped, does not
     * stop/start any adapter, and is never called implicitly by [start] or
     * [stop].  A host adapter owns the actual `MmpNode.flush()` call and any
     * Keystore/Keychain policy; failures are propagated unchanged.
     */
    public fun flushPersistence() {
        driveLoopOwner.check()
        persistenceSink.flushPersistence()
    }

    /**
     * Undo callbacks that may have arrived while a native scanner was
     * starting.  A platform callback can be delivered synchronously from
     * `Scanner.start`, or the initial drain can fail after a link was
     * admitted.  Leaving those links in [activeLinks] would poison the next
     * `start()` call and could retain native GATT resources after startup has
     * already reported failure.
     */
    private fun rollbackStartLinks() {
        val links = activeLinks.toList()
        links.forEach { runCatching { node.linkDown(it) } }
        activeLinks.clear()
        links.forEach { runCatching { requestPhysicalClose(it) } }
        closeRequested.clear()
        pending.clear()
        advertisedBeacon = null
    }

    /**
     * Advance the node using the injected monotonic time and drain output.
     * Equal timestamps are allowed; backwards time is rejected before the node
     * sees it.
     */
    public fun pump() {
        driveLoopOwner.check()
        requireRunning()
        advance(clock.nowMs(), emit = true)
    }

    /** Deterministic equivalent of [pump] for host/JVM tests. */
    public fun pumpAt(nowMs: Long) {
        driveLoopOwner.check()
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
        driveLoopOwner.check()
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
        driveLoopOwner.check()
        relayCapable = capable
        applyTier()
    }

    public fun onScanResult(result: ScanResult) {
        driveLoopOwner.check()
        if (!running) return
        if (result.serviceUuid?.equals(MmpBleService.SERVICE_UUID, ignoreCase = true) != true) return
        val payload = result.serviceData ?: return
        if (payload.isEmpty()) return
        node.onBeacon(payload.copyOf(), result.rssi)
    }

    public fun onLinkUp(
        link: Long,
        initiator: Boolean,
        quality: LinkQualityHint? = null,
    ) {
        driveLoopOwner.check()
        if (quality != null && !quality.isValid()) {
            // Reject before activeLinks or the Rust session is touched.  The
            // physical callback must still be torn down even when validation
            // fails before logical admission.
            runCatching { gatt.close(link) }
            throw IllegalArgumentException(
                "invalid link quality mtu=${quality.mtu} goodputBps=${quality.goodputBps}",
            )
        }
        // A callback can race with fail-closed shutdown.  It must never leave
        // a newly-created native link alive merely because the coordinator is
        // no longer accepting protocol work.
        if (!running || overloadLatched) {
            runCatching { gatt.close(link) }
            return
        }
        if (activeLinks.contains(link)) return
        if (activeLinks.size >= maxTrackedLinks) {
            latchOverload()
            runCatching { gatt.close(link) }
            return
        }
        if (!activeLinks.add(link)) return
        if (quality == null) {
            node.linkUp(link, initiator)
        } else {
            node.linkUp(link, initiator, quality)
        }
        drain()
    }

    public fun onLinkData(link: Long, bytes: ByteArray) {
        driveLoopOwner.check()
        // Late native callbacks after a fail-closed stop are stale input, not
        // a reason to resurrect protocol state or throw on the drive thread.
        if (!running || overloadLatched) return
        if (link !in activeLinks) return
        if (bytes.isEmpty()) return
        node.linkData(link, bytes.copyOf())
        drain()
    }

    public fun onLinkDown(link: Long) {
        driveLoopOwner.check()
        closeRequested.remove(link)
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
        observeHostDiagnostics()
        if (!overloadLatched) drainSend()
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
            requestPhysicalClose(link)
        }
    }

    /**
     * Issue one native close request per link.  Some platform callbacks are
     * synchronous while others are delayed; the de-duplication set keeps both
     * paths from issuing repeated teardown requests and remains bounded by
     * [maxTrackedLinks].
     */
    private fun requestPhysicalClose(link: Long) {
        if (closeRequested.add(link)) {
            try {
                gatt.close(link)
            } catch (error: Throwable) {
                // Do not permanently suppress a retry after a transient
                // adapter failure.  Callers decide whether this close is part
                // of normal draining (propagate) or fail-closed cleanup
                // (contain and continue with the remaining links).
                closeRequested.remove(link)
                throw error
            }
        }
    }

    /** Observe sticky Rust backpressure counters and fail closed on deltas. */
    private fun observeHostDiagnostics() {
        val closeDrops = node.closeActionsDropped()
        val eventDrops = node.eventQueueDropped()
        val closeDelta = closeDrops > observedCloseDrops
        val eventDelta = eventDrops > observedEventDrops
        observedCloseDrops = maxOf(observedCloseDrops, closeDrops)
        observedEventDrops = maxOf(observedEventDrops, eventDrops)
        if (closeDelta || eventDelta) latchOverload()
    }

    /**
     * Stop admitting links and tear down every locally tracked physical link.
     * The Rust node is told about the logical teardown before ownership is
     * released; the native adapter then receives the physical close request.
     * This is the bounded recovery path when a host-facing queue overflow
     * means individual close actions can no longer be trusted.
     */
    private fun latchOverload() {
        if (overloadLatched) return
        overloadLatched = true
        val links = activeLinks.toList()
        links.forEach { runCatching { node.linkDown(it) } }
        activeLinks.clear()
        pending.clear()
        links.forEach { runCatching { requestPhysicalClose(it) } }
        runCatching { scanner.stop() }
        runCatching { advertiser.stop() }
        advertisedBeacon = null
        running = false
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

    internal fun checkDriveLoopOwner() {
        driveLoopOwner.check()
    }

    private fun requireRunning() {
        check(running) { "MeshDrive is not running" }
    }
}

/**
 * SDK-free translation of one native GATT callback context into MeshDrive's
 * process-local link token.  The native adapter creates a fresh
 * [ConnectionIdentity] for every physical connection and retains the
 * resulting binding in each callback closure.  A delayed callback from a
 * retired Android handle is consequently ignored instead of reaching the
 * protocol session for a replacement connection.
 */
public class GattLinkBridge(
    private val drive: MeshDrive,
    private val bindings: ConnectionBindingRegistry = ConnectionBindingRegistry(),
) {
    /** Admit a physical link, or return null so the adapter can close it. */
    public fun onConnected(
        identity: ConnectionIdentity,
        initiator: Boolean,
        quality: LinkQualityHint? = null,
    ): ConnectionBindingRegistry.ConnectionBinding? {
        drive.checkDriveLoopOwner()
        val binding = bindings.open(identity) ?: return null
        if (!drive.isRunning || drive.isOverloaded) {
            bindings.close(binding)
            return null
        }
        try {
            drive.onLinkUp(binding.link, initiator, quality)
            if (!drive.isRunning || drive.isOverloaded) {
                bindings.close(binding)
                return null
            }
            return binding
        } catch (error: Throwable) {
            // A native adapter can fail while the first drain is negotiating
            // MTU or publishing a notification.  Do not retain a binding for
            // a protocol link that was only partially admitted.
            bindings.close(binding)
            runCatching { drive.onLinkDown(binding.link) }
            throw error
        }
    }

    /** Forward data only while the callback context remains current. */
    public fun onData(binding: ConnectionBindingRegistry.ConnectionBinding, bytes: ByteArray) {
        drive.checkDriveLoopOwner()
        if (bindings.isCurrent(binding)) drive.onLinkData(binding.link, bytes)
    }

    /** Retire first, then forward one logical link-down to the coordinator. */
    public fun onDisconnected(binding: ConnectionBindingRegistry.ConnectionBinding) {
        drive.checkDriveLoopOwner()
        if (bindings.close(binding)) drive.onLinkDown(binding.link)
    }

    public fun activeConnections(): Int = bindings.size()
}
