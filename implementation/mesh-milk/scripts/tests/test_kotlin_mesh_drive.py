import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
SOURCE = ROOT / "platform" / "kotlin-mesh-drive" / "src" / "main" / "kotlin" / "org" / "mesh_milk" / "platform" / "MeshDrive.kt"
README = ROOT / "platform" / "kotlin-mesh-drive" / "README.md"


class KotlinMeshDriveSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")

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
        self.assertIn("links.forEach { runCatching { gatt.close(it) } }", helper)
        self.assertIn("pending.clear()", helper)
        self.assertLess(helper.index("activeLinks.clear()"), helper.index("gatt.close(it)"))

    def test_stop_requests_native_teardown_after_releasing_protocol_links(self) -> None:
        start = self.source.index("public fun stop()")
        end = self.source.index("public fun pump()", start)
        body = self.source[start:end]
        self.assertIn("val links = activeLinks.toList()", body)
        self.assertIn("links.forEach { node.linkDown(it) }", body)
        self.assertIn("links.forEach { gatt.close(it) }", body)
        self.assertLess(body.index("node.linkDown(it)"), body.index("activeLinks.clear()"))
        self.assertLess(body.index("activeLinks.clear()"), body.index("gatt.close(it)"))

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
        self.assertIn("node.pollSend()", self.source)
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
        self.assertIn("fun close(link: Long)", self.source)
        self.assertIn("node.pollClose()", self.source)
        self.assertIn("gatt.close(link)", self.source)
        self.assertIn("pending.removeIf { it.link == link }", self.source)
        self.assertLess(self.source.index("drainClose()"), self.source.index("drainSend()"))

    def test_transport_diagnostics_are_exposed_and_link_down_drains_all_output(self) -> None:
        self.assertIn("fun closeActionsDropped(): Long", self.source)
        self.assertIn("fun eventQueueDropped(): Long", self.source)
        self.assertIn("node.closeActionsDropped()", self.source)
        self.assertIn("node.eventQueueDropped()", self.source)
        link_down = self.source.index("public fun onLinkDown")
        advance = self.source.index("private fun advance", link_down)
        self.assertIn("drain()", self.source[link_down:advance])

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


if __name__ == "__main__":
    unittest.main()
