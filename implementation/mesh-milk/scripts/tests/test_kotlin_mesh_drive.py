import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
SOURCE = ROOT / "platform" / "kotlin-mesh-drive" / "src" / "main" / "kotlin" / "org" / "mesh_milk" / "platform" / "MeshDrive.kt"
TOKEN_ALLOCATOR = ROOT / "platform" / "kotlin-mesh-drive" / "src" / "main" / "kotlin" / "org" / "mesh_milk" / "platform" / "LinkTokenAllocator.kt"
BINDING_REGISTRY = ROOT / "platform" / "kotlin-mesh-drive" / "src" / "main" / "kotlin" / "org" / "mesh_milk" / "platform" / "ConnectionBindingRegistry.kt"
README = ROOT / "platform" / "kotlin-mesh-drive" / "README.md"
FIXTURE_DIR = ROOT / "platform" / "kotlin-mesh-drive"
FIXTURE_SETTINGS = FIXTURE_DIR / "settings.gradle.kts"
FIXTURE_BUILD = FIXTURE_DIR / "build.gradle.kts"
FIXTURE_TEST = (
    FIXTURE_DIR
    / "src"
    / "test"
    / "kotlin"
    / "org"
    / "mesh_milk"
    / "platform"
    / "MeshDriveConsumerTest.kt"
)


class KotlinMeshDriveSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")
        cls.consumer_tests = FIXTURE_TEST.read_text(encoding="utf-8")

    def test_source_is_android_sdk_free(self) -> None:
        lowered = self.source.lower()
        self.assertNotIn("import android.", lowered)
        self.assertNotIn("import androidx.", lowered)
        self.assertNotIn("import kotlinx.coroutines", lowered)

    def test_dependencies_are_injected(self) -> None:
        for interface_name in (
            "MonotonicClock",
            "Advertiser",
            "Scanner",
            "GattAdapter",
        ):
            self.assertIn(f"interface {interface_name}", self.source)
        for parameter in ("clock: MonotonicClock", "advertiser: Advertiser", "scanner: Scanner", "gatt: GattAdapter"):
            self.assertIn(parameter, self.source)

    def test_duty_cycle_request_is_validated_and_applied_before_radio_start(self) -> None:
        for symbol in ("DutyCycleRequest", "DutyCycleSink", "applyDutyCycle", "dutyCycleRequest.validate()"):
            self.assertIn(symbol, self.source)
        for default in (
            "baseAdvertisementIntervalMs: Int = 5_000",
            "advertisementJitterPercent: Int = 10",
            "scanWindowMs: Int = 30",
            "scanIntervalMs: Int = 1_280",
            "burstScanDurationMs: Int = 30_000",
            "perSessionAirtimeCapMs: Int = 60_000",
        ):
            self.assertIn(default, self.source)
        start = self.source.index("public fun start()")
        body = self.source[start:self.source.index("public fun stop()", start)]
        self.assertLess(body.index("dutyCycleRequest.validate()"), body.index("advertiser.start"))
        self.assertIn("dutyCycleSink.applyDutyCycle(dutyCycleRequest.copy())", body)

    def test_duty_cycle_bounds_are_explicit(self) -> None:
        for bound in (
            "2000..10000",
            "exactly 10",
            "30..scanIntervalMs",
            "at most 2.5%",
            "0..30000",
            "0..6",
            "15..30",
            "exactly 4000",
            "exactly 60000",
        ):
            self.assertIn(bound, self.source)

    def test_persistence_flush_is_explicit_host_owned_and_not_tied_to_stop(self) -> None:
        for symbol in ("PersistenceSink", "flushPersistence", "MmpNode.flush()"):
            self.assertIn(symbol, self.source)
        constructor = self.source[self.source.index("public class MeshDrive("):self.source.index(") {", self.source.index("public class MeshDrive("))]
        self.assertIn("persistenceSink: PersistenceSink = PersistenceSink { }", constructor)
        method_start = self.source.rindex("public fun flushPersistence()")
        method_end = self.source.index("    /**", method_start)
        method = self.source[method_start:method_end]
        self.assertIn("driveLoopOwner.check()", method)
        self.assertIn("persistenceSink.flushPersistence()", method)
        stop_start = self.source.index("public fun stop()")
        stop_end = self.source.index("    /**", stop_start)
        self.assertNotIn("flushPersistence", self.source[stop_start:stop_end])
        self.assertIn("Keystore durability", self.readme)
        self.assertIn("in-memory no-op", self.readme)

    def test_secret_state_boundary_is_opaque_explicit_and_fail_closed(self) -> None:
        for symbol in (
            "SecretStatePersistence",
            "restoreSecretState",
            "persistSecretState",
            "UnavailableSecretStatePersistence",
        ):
            self.assertIn(symbol, self.source)
        constructor = self.source[self.source.index("public class MeshDrive("):self.source.index(") {", self.source.index("public class MeshDrive("))]
        self.assertIn("secretStatePersistence: SecretStatePersistence = UnavailableSecretStatePersistence()", constructor)
        self.assertIn("require(generation == null || generation > 0L)", self.source)
        self.assertIn("require(generation > 0L)", self.source)
        self.assertNotIn("ByteArray", self.source[self.source.index("public interface SecretStatePersistence"):self.source.index("/** Minimal node/FFI", self.source.index("public interface SecretStatePersistence"))])

    def test_secret_state_operations_are_lifecycle_gated(self) -> None:
        restore_start = self.source.index("    public fun restoreSecretState(): Long? {")
        restore_end = self.source.index("    /**", restore_start)
        restore = self.source[restore_start:restore_end]
        persist_start = self.source.index("    public fun persistSecretState(): Long {")
        persist_end = self.source.index("    /**", persist_start)
        persist = self.source[persist_start:persist_end]
        self.assertIn("check(!running)", restore)
        self.assertIn("before MeshDrive.start", restore)
        self.assertIn("check(!running)", persist)
        self.assertIn("after MeshDrive.stop", persist)
        self.assertIn("secretStateOperationsAreRejectedWhileRadioIsRunning", self.consumer_tests)

    def test_start_rolls_back_partial_radio_start(self) -> None:
        """A failed startup must not leave the drive claiming to be active."""

        start = self.source.index("public fun start()")
        stop = self.source.index("public fun stop()", start)
        body = self.source[start:stop]
        self.assertIn("var advertiserStarted = false", body)
        self.assertIn("var scannerStarted = false", body)
        self.assertIn("runCatching { advertiser.stop() }", body)
        self.assertIn("runCatching { scanner.stop() }", body)
        self.assertIn("rollbackStartLinks()", body)
        self.assertIn("running = false", body)
        self.assertIn("throw error", body)
        self.assertLess(body.index("advertiser.start"), body.index("scanner.start"))
        self.assertLess(body.index("scanner.start"), body.index("running = false"))

    def test_failed_start_releases_links_admitted_during_native_startup(self) -> None:
        """A failed native start must not poison the next start with stale links."""

        helper = self.source[self.source.index("private fun rollbackStartLinks()") :]
        self.assertIn("val links = activeLinks.toList()", helper)
        self.assertIn("links.forEach { runCatching { node.linkDown(it) } }", helper)
        self.assertIn("activeLinks.clear()", helper)
        self.assertIn("links.forEach { runCatching { requestPhysicalClose(it) } }", helper)
        self.assertIn("pending.clear()", helper)
        self.assertLess(helper.index("activeLinks.clear()"), helper.index("requestPhysicalClose(it)"))

    def test_stop_requests_native_teardown_after_releasing_protocol_links(self) -> None:
        start = self.source.index("public fun stop()")
        end = self.source.index("public fun pump()", start)
        body = self.source[start:end]
        self.assertIn("val links = activeLinks.toList()", body)
        self.assertIn("links.forEach { node.linkDown(it) }", body)
        self.assertIn("links.forEach { runCatching { requestPhysicalClose(it) } }", body)
        self.assertLess(body.index("node.linkDown(it)"), body.index("activeLinks.clear()"))
        self.assertLess(body.index("activeLinks.clear()"), body.index("requestPhysicalClose(it)"))

    def test_stop_contains_adapter_failures_and_finishes_state_cleanup(self) -> None:
        """One native teardown failure must not strand other links or discovery."""

        start = self.source.index("public fun stop()")
        end = self.source.index("public fun pump()", start)
        body = self.source[start:end]
        self.assertIn("links.forEach { runCatching { requestPhysicalClose(it) } }", body)
        self.assertIn("runCatching { scanner.stop() }", body)
        self.assertIn("runCatching { advertiser.stop() }", body)
        self.assertLess(body.index("closeRequested.clear()"), body.index("running = false"))
        self.assertLess(body.index("runCatching { scanner.stop() }"), body.index("running = false"))
        self.assertLess(body.index("runCatching { advertiser.stop() }"), body.index("running = false"))

    def test_monotonic_tick_is_checked_before_node_tick(self) -> None:
        self.assertIn("nowMs >= (previous ?: Long.MIN_VALUE)", self.source)
        guard = self.source.index("nowMs >= (previous ?: Long.MIN_VALUE)")
        node_tick = self.source.index("node.tick(nowMs)", guard)
        self.assertLess(guard, node_tick)
        self.assertIn("advance(clock.nowMs(), emit = true)", self.source)

    def test_rotating_beacon_has_an_explicit_refresh_hook(self) -> None:
        """Long-lived mobile hosts must be able to replace the startup beacon."""

        start = self.source.index("public fun refreshBeacon(nowMs: Long)")
        end = self.source.index("public fun setForegroundRelayCapable", start)
        body = self.source[start:end]
        self.assertIn("node.beacon(nowMs)", body)
        self.assertIn("advertiser.start(beacon.copyOf())", body)
        self.assertIn("advertisedBeacon?.contentEquals(beacon) == true", body)
        self.assertIn("advertisedBeacon = beacon.copyOf()", body)
        self.assertIn("nowMs >= (previous ?: Long.MIN_VALUE)", body)
        self.assertNotIn("node.tick(nowMs)", body)
        self.assertIn("refreshBeacon(nowMs)", self.readme)

    def test_poll_send_is_pull_based_and_backpressure_is_retained(self) -> None:
        self.assertIn("node.pollSendBatch(MeshNode.MAX_POLL_BATCH)", self.source)
        self.assertIn("fun pollSend(): OutboundFrame?", self.source)
        self.assertIn("fun pollSendBatch(maxItems: Int)", self.source)
        self.assertIn("node.pollEventBatch(MeshNode.MAX_POLL_BATCH)", self.source)
        self.assertIn("fun pollEventBatch(maxItems: Int)", self.source)
        self.assertIn("pending.addLast(pendingFrame)", self.source)
        self.assertIn("if (!accepted) return", self.source)
        self.assertIn("notifyFragment", self.source)

    def test_mtu_failure_cannot_drop_a_polled_frame(self) -> None:
        """A transient adapter MTU failure must leave the envelope retryable."""

        self.assertIn("var fragmentSize: Int? = null", self.source)
        retained = self.source.index("pending.addLast(pendingFrame)")
        lookup = self.source.index("gatt.mtu(frame.link)")
        self.assertLess(retained, lookup)
        self.assertIn("if (frame.fragmentSize == null)", self.source)

    def test_transport_close_actions_are_drained_before_send(self) -> None:
        self.assertIn("fun pollClose(): Long?", self.source)
        self.assertIn("node.pollCloseBatch(MeshNode.MAX_POLL_BATCH)", self.source)
        self.assertIn("fun pollCloseBatch(maxItems: Int)", self.source)
        self.assertIn("fun close(link: Long)", self.source)
        self.assertIn("gatt.close(link)", self.source)
        self.assertIn("pending.removeIf { it.link == link }", self.source)
        self.assertLess(self.source.index("drainClose()"), self.source.index("drainSend()"))

    def test_transport_diagnostics_are_exposed_and_link_down_drains_all_output(self) -> None:
        self.assertIn("fun closeActionsDropped(): Long", self.source)
        self.assertIn("fun eventQueueDropped(): Long", self.source)
        self.assertIn("node.closeActionsDropped()", self.source)
        self.assertIn("node.eventQueueDropped()", self.source)
        self.assertIn("observeHostDiagnostics()", self.source)
        self.assertIn("latchOverload()", self.source)
        self.assertIn("overloadLatched", self.source)
        link_down = self.source.index("public fun onLinkDown")
        advance = self.source.index("private fun advance", link_down)
        self.assertIn("drain()", self.source[link_down:advance])

    def test_overload_latch_blocks_outbound_drain(self) -> None:
        """Fail-closed host overload must not send frames after diagnostics trip."""

        drain = self.source[self.source.index("private fun drain()"): self.source.index("private fun drainClose()")]
        self.assertIn("observeHostDiagnostics()", drain)
        self.assertIn("if (!overloadLatched) drainSend()", drain)

    def test_admission_and_overload_paths_close_native_links_and_bound_tracking(self) -> None:
        """A denied or saturated callback must not grow host-side link state."""

        self.assertIn("private val maxTrackedLinks: Int = 64", self.source)
        self.assertIn("if (activeLinks.size >= maxTrackedLinks)", self.source)
        self.assertIn("if (!running || overloadLatched)", self.source)
        self.assertIn("runCatching { gatt.close(link) }", self.source)
        self.assertIn("if (!running || overloadLatched) return", self.source)
        self.assertIn("activeLinks.clear()", self.source)
        self.assertIn("pending.clear()", self.source)
        self.assertIn("runCatching { node.linkDown(it) }", self.source)
        self.assertIn("runCatching { requestPhysicalClose(it) }", self.source)

    def test_duplicate_link_up_is_idempotent_before_capacity_check(self) -> None:
        """A repeated native callback must not crash or trip the overload latch."""

        start = self.source.index("public fun onLinkUp")
        end = self.source.index("public fun onLinkData", start)
        body = self.source[start:end]
        self.assertIn("if (activeLinks.contains(link)) return", body)
        self.assertNotIn("check(activeLinks.add(link))", body)
        self.assertLess(
            body.index("if (activeLinks.contains(link)) return"),
            body.index("if (activeLinks.size >= maxTrackedLinks)"),
        )

    def test_close_requests_are_deduplicated_until_disconnect_callback(self) -> None:
        self.assertIn("private val closeRequested = LinkedHashSet<Long>()", self.source)
        self.assertIn("if (closeRequested.add(link))", self.source)
        self.assertIn("closeRequested.remove(link)", self.source)
        self.assertLess(self.source.index("requestPhysicalClose(link)"), self.source.index("private fun requestPhysicalClose"))

    def test_mtu_three_fragmentation_is_explicit(self) -> None:
        self.assertIn("ATT_HEADER_BYTES: Int = 3", self.source)
        self.assertIn("fragmentSize = mtu - ATT_HEADER_BYTES", self.source)
        self.assertIn("copyOfRange(frame.offset, end)", self.source)
        self.assertIn("mtu >= MIN_ATT_MTU", self.source)

    def test_link_lifecycle_and_leaf_fallback_are_present(self) -> None:
        for method in ("onLinkUp", "onLinkData", "onLinkDown", "node.linkUp", "node.linkData", "node.linkDown"):
            self.assertIn(method, self.source)
        self.assertIn("node.setTier(if (relayCapable) MeshTier.RELAY else MeshTier.LEAF)", self.source)
        self.assertIn("setForegroundRelayCapable", self.source)
        self.assertIn("pending.removeIf { it.link == link }", self.source)

    def test_link_token_allocator_is_monotonic_and_fails_before_wrap(self) -> None:
        """Native adapters must not reuse connection IDs while callbacks are pending."""

        self.assertTrue(TOKEN_ALLOCATOR.exists())
        allocator = TOKEN_ALLOCATOR.read_text(encoding="utf-8")
        self.assertIn("public class LinkTokenAllocator", allocator)
        self.assertIn("require(firstToken > 0L)", allocator)
        self.assertIn("Long.MAX_VALUE", allocator)
        self.assertIn("throw IllegalStateException", allocator)
        self.assertIn("@Synchronized", allocator)
        self.assertIn("nextToken = token + 1L", allocator)
        self.assertIn("LinkTokenAllocator", self.readme)

    def test_connection_binding_registry_fences_stale_callbacks_and_bounds_live_entries(self) -> None:
        """Callback context, not a reused handle, owns the link lifecycle."""

        self.assertTrue(BINDING_REGISTRY.exists())
        registry = BINDING_REGISTRY.read_text(encoding="utf-8")
        self.assertIn("public class ConnectionBindingRegistry", registry)
        self.assertIn("public class ConnectionIdentity private constructor()", registry)
        self.assertIn("public fun create(): ConnectionIdentity", registry)
        self.assertIn("IdentityHashMap", registry)
        self.assertIn("public fun open(identity: ConnectionIdentity): ConnectionBinding?", registry)
        self.assertIn("public fun close(binding: ConnectionBinding): Boolean", registry)
        self.assertIn("public fun isCurrent(binding: ConnectionBinding): Boolean", registry)
        self.assertIn("private val active", registry)
        self.assertIn("maxBindings > 0", registry)
        self.assertIn("if (active.size >= maxBindings) return null", registry)
        self.assertIn("current !== binding", registry)
        self.assertIn("=== binding", registry)
        self.assertLess(
            registry.index("active[identity]?.let { return it }"),
            registry.index("if (active.size >= maxBindings) return null"),
        )
        self.assertIn("ConnectionBindingRegistry", self.readme)

    def test_link_callbacks_forward_before_draining_host_output(self) -> None:
        """Keep callback ordering compatible with the FFI single-owner contract.

        The Rust FFI exposes a pull-based ``pollSend``/``pollEvent`` surface;
        a native callback must first inject the transport event and only then
        drain the resulting output.  This source-level guard catches a future
        refactor that drains before ``linkUp``/``linkData``/``linkDown`` and
        would therefore reorder handshake or close events at the mobile
        boundary without requiring an Android SDK or BLE hardware.
        """

        def method_body(start: str, end: str) -> str:
            begin = self.source.index(start)
            finish = self.source.index(end, begin)
            return self.source[begin:finish]

        link_up = method_body("public fun onLinkUp", "public fun onLinkData")
        self.assertLess(link_up.index("node.linkUp(link, initiator)"), link_up.index("drain()"))

        link_data = method_body("public fun onLinkData", "public fun onLinkDown")
        self.assertLess(link_data.index("node.linkData(link, bytes.copyOf())"), link_data.index("drain()"))

        link_down = method_body("public fun onLinkDown", "private fun advance")
        self.assertLess(link_down.index("pending.removeIf { it.link == link }"), link_down.index("node.linkDown(link)"))
        self.assertLess(link_down.index("node.linkDown(link)"), link_down.index("drain()"))

    def test_evidence_boundary_is_documented(self) -> None:
        self.assertIn("Android-SDK-free", self.readme)
        self.assertIn("evidence of GATT", self.readme)

    def test_jvm_consumer_fixture_is_pinned_and_sdk_free(self) -> None:
        """Keep the downstream compilation gate isolated from Android tooling."""

        self.assertTrue(FIXTURE_SETTINGS.exists())
        self.assertTrue(FIXTURE_BUILD.exists())
        self.assertTrue(FIXTURE_TEST.exists())
        build = FIXTURE_BUILD.read_text(encoding="utf-8")
        self.assertIn('kotlin("jvm") version "2.1.21"', build)
        self.assertIn('testImplementation(kotlin("test"))', build)
        self.assertNotIn("com.android", build)
        self.assertNotIn("androidx", build)

    def test_jvm_consumer_fixture_type_checks_public_coordinator_surface(self) -> None:
        consumer = FIXTURE_TEST.read_text(encoding="utf-8")
        for public_type in (
            "MeshDrive",
            "MeshNode",
            "MonotonicClock",
            "Advertiser",
            "Scanner",
            "GattAdapter",
        ):
            self.assertIn(public_type, consumer)
        self.assertIn("drive.start()", consumer)
        self.assertIn("drive.stop()", consumer)


if __name__ == "__main__":
    unittest.main()
