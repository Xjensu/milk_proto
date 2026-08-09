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

    def test_monotonic_tick_is_checked_before_node_tick(self) -> None:
        self.assertIn("nowMs >= (previous ?: Long.MIN_VALUE)", self.source)
        guard = self.source.index("nowMs >= (previous ?: Long.MIN_VALUE)")
        node_tick = self.source.index("node.tick(nowMs)", guard)
        self.assertLess(guard, node_tick)
        self.assertIn("advance(clock.nowMs(), emit = true)", self.source)

    def test_poll_send_is_pull_based_and_backpressure_is_retained(self) -> None:
        self.assertIn("node.pollSend()", self.source)
        self.assertIn("pending.addLast(pendingFrame)", self.source)
        self.assertIn("if (!accepted) return", self.source)
        self.assertIn("notifyFragment", self.source)

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
        self.assertLess(link_down.index("node.linkDown(link)"), link_down.index("drainEvents()"))

    def test_evidence_boundary_is_documented(self) -> None:
        self.assertIn("Android-SDK-free", self.readme)
        self.assertIn("evidence of GATT", self.readme)


if __name__ == "__main__":
    unittest.main()
