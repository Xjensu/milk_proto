import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
SOURCE = ROOT / "platform" / "swift-mesh-drive" / "Sources" / "MeshDrive.swift"
SHELL_SOURCE = ROOT / "platform" / "swift-mesh-drive" / "Sources" / "MeshRadioShell.swift"
README = ROOT / "platform" / "swift-mesh-drive" / "README.md"
PACKAGE = ROOT / "platform" / "swift-mesh-drive" / "Package.swift"
CONSUMER_TESTS = ROOT / "platform" / "swift-mesh-drive" / "Tests" / "MeshDriveTests" / "MeshDriveConsumerTests.swift"


class SwiftMeshDriveSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE.read_text(encoding="utf-8")
        cls.shell_source = SHELL_SOURCE.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")
        cls.package = PACKAGE.read_text(encoding="utf-8")
        cls.consumer_tests = CONSUMER_TESTS.read_text(encoding="utf-8")

    def test_swift_package_exposes_sdk_free_library_and_consumer_target(self) -> None:
        self.assertIn("swift-tools-version: 5.7", self.package)
        self.assertIn('.library(\n            name: "MeshDrive"', self.package)
        self.assertIn('.target(\n            name: "MeshDrive"', self.package)
        self.assertIn('.testTarget(\n            name: "MeshDriveTests"', self.package)
        self.assertIn('path: "Tests/MeshDriveTests"', self.package)
        self.assertNotIn("CoreBluetooth", self.package)

    def test_consumer_fixture_imports_only_public_meshdrive_api(self) -> None:
        self.assertIn("import MeshDrive", self.consumer_tests)
        self.assertNotIn("@testable import", self.consumer_tests)
        self.assertIn("MeshDrive(", self.consumer_tests)
        self.assertIn("try drive.start()", self.consumer_tests)
        self.assertIn("drive.stop()", self.consumer_tests)

    def test_source_is_corebluetooth_free_and_injected(self) -> None:
        lowered = self.source.lower()
        self.assertNotIn("import corebluetooth", lowered)
        for protocol in ("MeshNode", "MonotonicClock", "Advertiser", "Scanner", "GattAdapter"):
            self.assertIn(f"protocol {protocol}", self.source)

    def test_scan_admission_requires_observed_mmp_service_uuid(self) -> None:
        self.assertIn("public enum MmpBleService", self.source)
        self.assertIn("serviceUUID = \"6d6d7031-0000-4d4d-5031-6d6573686d6c\"", self.source)
        self.assertIn("serviceUuid: String? = nil", self.source)
        start = self.source.index("public func onScanResult")
        end = self.source.index("public func onLinkUp", start)
        body = self.source[start:end]
        self.assertIn("let observedServiceUuid = result.serviceUuid", body)
        self.assertIn("observedServiceUuid.lowercased() == MmpBleService.serviceUUID", body)
        self.assertIn("testScanResultRequiresExactMmpServiceUuid", self.consumer_tests)
        self.assertIn("testScanResultEmptyPayloadAndStoppedDriveRemainIgnored", self.consumer_tests)
        self.assertIn("serviceUuid: MmpBleService.serviceUUID.uppercased()", self.consumer_tests)
        self.assertIn('serviceUuid: \"wrong\"', self.consumer_tests)
        self.assertIn("Native CoreBluetooth adapters SHOULD install a targeted scan filter", self.readme)
        self.assertIn("remains fail-closed", self.readme)

    def test_duty_cycle_request_is_validated_and_applied_before_radio_start(self) -> None:
        for symbol in ("MeshDutyCycleRequest", "DutyCycleSink", "applyDutyCycle", "try dutyCycleRequest.validate()"):
            self.assertIn(symbol, self.source)
        for default in ("baseAdvertisementIntervalMs: Int = 5_000", "advertisementJitterPercent: Int = 10", "scanWindowMs: Int = 30", "scanIntervalMs: Int = 1_280", "burstScanDurationMs: Int = 30_000"):
            self.assertIn(default, self.source)
        start = self.source.index("public func start()")
        body = self.source[start:self.source.index("public func stop()", start)]
        self.assertLess(body.index("try dutyCycleRequest.validate()"), body.index("try advertiser.start"))

    def test_duty_cycle_bounds_and_consumer_tests_are_explicit(self) -> None:
        for bound in ("2_000...10_000", "== 10", "30...scanIntervalMs", "0...30_000", "0...6", "15...30", "4_000...32_000"):
            self.assertIn(bound, self.source)
        for test in ("testDutyCycleDefaultsMatchMmpBaselineAndApplyOnceBeforeRadioStart", "testDutyCycleAcceptsValidCandidate", "testDutyCycleRejectsInvalidValuesBeforeRadioStart"):
            self.assertIn(test, self.consumer_tests)

    def test_persistence_flush_is_explicit_host_owned_and_forwarded_by_shell(self) -> None:
        for symbol in ("PersistenceSink", "NoopPersistenceSink", "flushPersistence", "MmpNode.flush()"):
            self.assertIn(symbol, self.source)
        drive_start = self.source.index("public final class MeshDrive")
        constructor_start = self.source.index("public init(", drive_start)
        constructor = self.source[constructor_start:self.source.index("    ) {", constructor_start)]
        self.assertIn("persistenceSink: PersistenceSink = NoopPersistenceSink()", constructor)
        method_start = self.source.index("public func flushPersistence()")
        method_end = self.source.index("    public func pump()", method_start)
        method = self.source[method_start:method_end]
        self.assertIn("try persistenceSink.flushPersistence()", method)
        stop_start = self.source.index("public func stop()")
        stop_end = self.source.index("    public func flushPersistence()", stop_start)
        self.assertNotIn("flushPersistence", self.source[stop_start:stop_end])
        shell_start = self.shell_source.index("public func flushPersistence()")
        shell_method = self.shell_source[shell_start:self.shell_source.index("    ///", shell_start)]
        self.assertIn("try drive.flushPersistence()", shell_method)
        self.assertIn("Keychain", self.readme)
        self.assertIn("durability", self.readme)
        self.assertIn("in-memory no-op", self.readme)
        self.assertIn("flushPersistence", self.consumer_tests)

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
