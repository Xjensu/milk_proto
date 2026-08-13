import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).parents[2]


@dataclass(frozen=True)
class PlatformPersistenceContract:
    name: str
    source: Path
    consumer_tests: Path
    interface_start: str
    unavailable_end: str
    restore_start: str
    persist_start: str
    method_end: str
    default_provider: str
    provider_gate_tokens: tuple[tuple[str, tuple[str, ...]], ...]
    consumer_tokens: tuple[str, ...]


CONTRACTS = (
    PlatformPersistenceContract(
        name="Kotlin",
        source=ROOT
        / "platform"
        / "kotlin-mesh-drive"
        / "src"
        / "main"
        / "kotlin"
        / "org"
        / "mesh_milk"
        / "platform"
        / "MeshDrive.kt",
        consumer_tests=ROOT
        / "platform"
        / "kotlin-mesh-drive"
        / "src"
        / "test"
        / "kotlin"
        / "org"
        / "mesh_milk"
        / "platform"
        / "MeshDriveConsumerTest.kt",
        interface_start="public interface SecretStatePersistence {",
        unavailable_end="/** Minimal node/FFI surface needed by the drive loop. */",
        restore_start="    public fun restoreSecretState(): Long? {",
        persist_start="    public fun persistSecretState(): Long {",
        method_end="    /**",
        default_provider="secretStatePersistence: SecretStatePersistence = UnavailableSecretStatePersistence()",
        provider_gate_tokens=(
            (
                "restore",
                ("check(!running)", "before MeshDrive.start", "check(!hasCompletedStart)"),
            ),
            (
                "persist",
                ("check(!running)", "after MeshDrive.stop", "check(hasCompletedStart)"),
            ),
        ),
        consumer_tokens=(
            "secretStatePersistenceIsExplicitAndReturnsOnlyGenerationMetadata",
            "secretStateOperationsAreRejectedWhileRadioIsRunning",
            "secretStateRestoreIsRejectedAfterSuccessfulRadioCycle",
        ),
    ),
    PlatformPersistenceContract(
        name="Swift",
        source=ROOT / "platform" / "swift-mesh-drive" / "Sources" / "MeshDrive.swift",
        consumer_tests=ROOT
        / "platform"
        / "swift-mesh-drive"
        / "Tests"
        / "MeshDriveTests"
        / "MeshDriveConsumerTests.swift",
        interface_start="public protocol SecretStatePersistence {",
        unavailable_end="public enum MeshSecretStateError: Error, Equatable {",
        restore_start="    public func restoreSecretState() throws -> UInt64? {",
        persist_start="    public func persistSecretState() throws -> UInt64 {",
        method_end="    public func pump() throws {",
        default_provider="secretStatePersistence: SecretStatePersistence = UnavailableSecretStatePersistence()",
        provider_gate_tokens=(
            (
                "restore",
                ("guard !running else", "guard !hasCompletedStart else", "invalidLifecycle"),
            ),
            (
                "persist",
                ("guard !running else", "guard hasCompletedStart else", "invalidLifecycle"),
            ),
        ),
        consumer_tokens=(
            "testSecretStatePersistenceIsExplicitAndReturnsOnlyGenerationMetadata",
            "testSecretStateRestoreIsRejectedAfterRadioStartup",
            "testSecretStatePersistIsRejectedBeforeFirstSuccessfulRadioCycle",
        ),
    ),
)


def _interface_block(source: str, start: str) -> str:
    """Return only the provider declaration, excluding its default class."""

    start_index = source.index(start)
    end_index = source.index("\n}", start_index + len(start)) + len("\n}")
    return source[start_index:end_index]


def _slice_between(source: str, start: str, end: str) -> str:
    start_index = source.index(start)
    end_index = source.index(end, start_index + len(start))
    return source[start_index:end_index]


class MobilePlatformPersistenceContractTests(unittest.TestCase):
    def test_both_platforms_expose_the_same_opaque_generation_boundary(self) -> None:
        """Kotlin and Swift must not grow language-specific secret byte APIs."""

        for contract in CONTRACTS:
            with self.subTest(platform=contract.name):
                source = contract.source.read_text(encoding="utf-8")
                interface = _interface_block(source, contract.interface_start)

                expected_signatures = {
                    "Kotlin": (
                        "public fun restoreSecretState(): Long?",
                        "public fun persistSecretState(): Long",
                    ),
                    "Swift": (
                        "func restoreSecretState() throws -> UInt64?",
                        "func persistSecretState() throws -> UInt64",
                    ),
                }[contract.name]
                for signature in expected_signatures:
                    self.assertIn(signature, interface)
                    self.assertEqual(interface.count(signature), 1)

                # The provider may return only generation metadata.  A raw
                # byte container in this interface would make plaintext or
                # ciphertext handling part of the mobile coordinator ABI.
                self.assertNotRegex(interface, r"ByteArray|\[\s*UInt8\s*\]|\bData\b")

    def test_defaults_and_lifecycle_gates_are_fail_closed_on_both_platforms(self) -> None:
        for contract in CONTRACTS:
            with self.subTest(platform=contract.name):
                source = contract.source.read_text(encoding="utf-8")
                consumers = contract.consumer_tests.read_text(encoding="utf-8")

                self.assertIn(contract.default_provider, source)
                unavailable = _slice_between(
                    source,
                    "UnavailableSecretStatePersistence",
                    contract.unavailable_end,
                )
                self.assertIn("restoreSecretState", unavailable)
                self.assertIn("persistSecretState", unavailable)
                self.assertRegex(unavailable, r"throw|fatalError")

                for operation, tokens in contract.provider_gate_tokens:
                    method = _slice_between(
                        source,
                        getattr(contract, f"{operation}_start"),
                        contract.method_end,
                    )
                    for token in tokens:
                        self.assertIn(token, method)

                for test_name in contract.consumer_tokens:
                    self.assertIn(test_name, consumers)


if __name__ == "__main__":
    unittest.main()
