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

    def test_evidence_boundary_is_documented(self) -> None:
        self.assertIn("Android-SDK-free", self.readme)
        self.assertIn("evidence of GATT", self.readme)


if __name__ == "__main__":
    unittest.main()
