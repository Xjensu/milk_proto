import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
SOURCE = ROOT / "platform" / "swift-mesh-drive" / "Sources" / "MeshDrive.swift"
README = ROOT / "platform" / "swift-mesh-drive" / "README.md"


class SwiftMeshDriveSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")

    def test_source_is_corebluetooth_free_and_injected(self) -> None:
        lowered = self.source.lower()
        self.assertNotIn("import corebluetooth", lowered)
        for protocol in ("MeshNode", "MonotonicClock", "Advertiser", "Scanner", "GattAdapter"):
            self.assertIn(f"protocol {protocol}", self.source)

    def test_startup_and_monotonic_guards_are_fail_closed(self) -> None:
        self.assertIn("try requireMonotonic(nowMs)", self.source)
        self.assertIn("rollbackStartLinks()", self.source)
        self.assertIn("running = false", self.source)
        self.assertIn("case clockMovedBackwards", self.source)

    def test_advertiser_start_failure_stops_partial_native_state(self) -> None:
        start = self.source.index("try advertiser.start(serviceData: beacon)")
        catch = self.source.index("} catch {", start)
        stop = self.source.index("try? advertiser.stop()", catch)
        throw = self.source.index("throw error", stop)
        outer = self.source.index("if advertiserStarted { try? advertiser.stop() }")

        self.assertLess(start, catch)
        self.assertLess(catch, stop)
        self.assertLess(stop, throw)
        self.assertLess(throw, outer)

    def test_scanner_start_failure_stops_partial_native_state_before_outer_rollback(self) -> None:
        start = self.source.index("try scanner.start")
        catch = self.source.index("} catch {", start)
        stop = self.source.index("try? scanner.stop()", catch)
        throw = self.source.index("throw error", stop)
        outer = self.source.index("if scannerStarted { try? scanner.stop() }")
        advertiser_rollback = self.source.index("if advertiserStarted { try? advertiser.stop() }")

        self.assertLess(start, catch)
        self.assertLess(catch, stop)
        self.assertLess(stop, throw)
        self.assertLess(throw, outer)
        self.assertLess(outer, advertiser_rollback)

    def test_close_diagnostics_and_send_order_are_explicit(self) -> None:
        self.assertLess(self.source.index("try drainClose()"), self.source.index("observeHostDiagnostics()"))
        self.assertLess(self.source.index("observeHostDiagnostics()"), self.source.index("try drainSend()"))
        self.assertIn("pollClose()", self.source)
        self.assertIn("closeActionsDropped()", self.source)
        self.assertIn("eventQueueDropped()", self.source)
        self.assertIn("latchOverload()", self.source)

    def test_fragmentation_retains_backpressure_and_mtu_three(self) -> None:
        self.assertIn("attHeaderBytes = 3", self.source)
        self.assertIn("minAttMtu = 23", self.source)
        self.assertIn("notifyFragment", self.source)
        self.assertIn("guard accepted else { return }", self.source)
        self.assertIn("frame.fragmentSize == nil", self.source)

    def test_late_callbacks_and_pending_link_cleanup_are_bounded(self) -> None:
        self.assertIn("maxTrackedLinks = 64", self.source)
        self.assertIn("guard running, !overloadLatched", self.source)
        self.assertIn("activeLinks.contains(link)", self.source)
        self.assertIn("!bytes.isEmpty", self.source)
        self.assertIn("else { return }", self.source)
        self.assertIn("pending.removeAll { $0.link == link }", self.source)
        self.assertIn("activeLinks.removeAll()", self.source)
        self.assertIn("closeRequested.removeAll()", self.source)
        self.assertIn("SDK-free", self.readme)
        self.assertIn("CoreBluetooth", self.readme)
        self.assertIn("not evidence", self.readme)

    def test_duplicate_link_up_is_idempotent_before_capacity_check(self) -> None:
        """A repeated native callback must not crash or trip the overload latch."""

        start = self.source.index("public func onLinkUp")
        end = self.source.index("public func onLinkData", start)
        body = self.source[start:end]
        self.assertIn("if activeLinks.contains(link) { return }", body)
        self.assertNotIn("precondition(activeLinks.insert(link).inserted", body)
        self.assertLess(
            body.index("if activeLinks.contains(link) { return }"),
            body.index("guard activeLinks.count < maxTrackedLinks"),
        )


if __name__ == "__main__":
    unittest.main()
