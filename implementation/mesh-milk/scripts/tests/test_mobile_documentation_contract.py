import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class MobileDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.api_reference = (
            REPO_ROOT
            / "docs"
            / "internal"
            / "release-evidence"
            / "API_REFERENCE.md"
        ).read_text(
            encoding="utf-8"
        )
        cls.implementation = (
            REPO_ROOT / "docs" / "implementation" / "mobile-integration.md"
        ).read_text(
            encoding="utf-8"
        )
        cls.mobile = (
            REPO_ROOT / "implementation" / "mesh-milk" / "MOBILE_INTEGRATION.md"
        ).read_text(encoding="utf-8")
        cls.react_native = (
            REPO_ROOT
            / "implementation"
            / "mesh-milk"
            / "REACT_NATIVE_INTEGRATION.md"
        ).read_text(encoding="utf-8")

    def test_checked_revocation_and_auth_cert_exports_are_documented(self) -> None:
        self.assertIn("node.tryRevokeIdentity(reason: u8)", self.api_reference)
        self.assertRegex(
            self.api_reference,
            r"node\.revokeIdentityWithSuccessor\([^\n]+\) -> bytes\s+// throws FfiError",
        )
        self.assertIn("mintAuthCert(suite, subject, scope", self.api_reference)
        self.assertNotIn("AuthCert` itself has no dedicated method yet", self.api_reference)
        self.assertIn("# Mobile and desktop integration", self.implementation)
        self.assertIn("tryRevokeIdentity(reason: number)", self.react_native)
        self.assertIn("tryRevokeIdentity(reason: u8)", self.mobile)

    def test_mobile_examples_match_ffi_constructor_and_transport_signatures(self) -> None:
        # Both exported constructors accept the host-owned identity seed. Do
        # not describe only the legacy constructor as the seed boundary.
        self.assertIn("at an `MmpNode` constructor", self.api_reference)
        self.assertIn("`new()` or `newWithStoreLimit()`", self.api_reference)
        self.assertNotIn("exactly once, inward, at `MmpNode.new()`", self.api_reference)
        self.assertIn(
            "MmpNode(Keystore.loadOrCreateSeed(), FfiTier.RELAY, filesDir.resolve(\"mmp\").path)",
            self.mobile,
        )
        self.assertNotIn(
            "MmpNode(Keystore.loadOrCreateSeed(), filesDir.resolve(\"mmp\").path, FfiTier.RELAY)",
            self.mobile,
        )
        self.assertNotIn("node.linkData(link, bytes, now)", self.mobile)
        self.assertNotIn("initiator: true, now())", self.mobile)
        self.assertNotIn("node.tick(Date.now())", self.api_reference)
        self.assertIn("newWithStoreLimit(seed: bytes32", self.api_reference)

    def test_reference_docs_cover_additive_ffi_surface(self) -> None:
        """Keep the three integration references aligned with mmp-ffi additions."""
        for name in (
            "ConversationPending",
            "betaSnapshot",
            "status",
            "capabilities",
            "metrics",
            "pollSendBatch",
            "pollEventBatch",
            "pollCloseBatch",
            "linkUpWithQuality",
        ):
            self.assertIn(name, self.api_reference, name)
            self.assertIn(name, self.mobile, name)
            self.assertIn(name, self.react_native, name)

        # Group application bodies carry the same type tag as direct bodies;
        # dropping it would make the bridge unable to dispatch custom payloads.
        self.assertRegex(
            self.api_reference,
            r"GroupMessage.*apptype.*body",
        )
        self.assertRegex(
            self.react_native,
            r"groupMessage.*apptype.*body",
        )

        # The old claim that link quality was unavailable must not return while
        # the checked additive method is part of the FFI contract.
        self.assertNotIn("is not yet exposed at the FFI boundary", self.api_reference)

    def test_binding_generation_commands_enable_uniffi_cli(self) -> None:
        """The documented UniFFI binary must be enabled by its Cargo feature."""
        for language in ("kotlin", "swift"):
            match = re.search(
                rf"(?ms)^cargo run -p mmp-ffi(?P<command>.*?--language\s+{language}\s+--out-dir[^\n]*)",
                self.mobile,
            )
            self.assertIsNotNone(match, language)
            command = " ".join(match.group(0).split())
            self.assertIn("--features cli", command, language)
            self.assertIn("--release", command, language)
            self.assertIn("--locked", command, language)
            self.assertIn("--bin uniffi-bindgen", command, language)
            self.assertIn(f"--language {language}", command, language)

    def test_persistence_and_group_return_contracts_are_aligned(self) -> None:
        """Keep mobile-facing docs aligned with the current SnapshotStore boundary."""
        docs = {
            "api": self.api_reference,
            "mobile": self.mobile,
            "react_native": self.react_native,
        }
        persistence_markers = (
            "seenindex",
            "receiptfilter",
            "authority admission state",
            "blocklist",
            "session/ratchet",
            "prekey",
            "platform-keystore",
            "not persist",
        )
        for name, document in docs.items():
            with self.subTest(document=name):
                document = document.lower()
                for marker in persistence_markers:
                    self.assertIn(marker, document, marker)

        # The API reference is the normative wording for each persisted record.
        self.assertIn(
            "| `SeenIndex` (dedup/replay window) | **Yes** |", self.api_reference
        )
        self.assertIn("| `ReceiptFilter` | **Yes** |", self.api_reference)
        self.assertIn("| Explicit local blocklist | **Yes** |", self.api_reference)
        self.assertIn(
            "| Session/ratchet/prekey secret state",
            self.api_reference,
        )

        # Group methods have deliberately different return shapes in every host
        # binding: invite is void, send returns a BundleID, and remove returns
        # the new epoch.
        group_shapes = {
            "api": (
                "node.groupInvite(gid: bytes16, peer: bytes16)",
                "node.groupSend(gid: bytes16, text: string) -> bytes16",
                "node.groupRemove(gid: bytes16, member: bytes16) -> u16",
            ),
            "mobile": (
                ".groupInvite(gid: bytes16, peer: bytes16)",
                ".groupSend(gid: bytes16, text: string) -> bytes16",
                ".groupRemove(gid: bytes16, member: bytes16) -> u16",
            ),
            "react_native": (
                "groupInvite(gid: GroupIdHex, peer: NodeIdHex): Promise<void>",
                "groupSend(gid: GroupIdHex, text: string): Promise<MsgIdHex>",
                "groupRemove(gid: GroupIdHex, member: NodeIdHex): Promise<number>",
            ),
        }
        for name, markers in group_shapes.items():
            with self.subTest(group_document=name):
                for marker in markers:
                    self.assertIn(marker, docs[name], marker)

        self.assertNotIn(
            ".groupInvite(gid, peer) / .groupSend(gid, text) / .groupRemove",
            self.mobile,
        )
        self.assertNotIn(
            "`SeenIndex`, receipts, and\nsession/ratchet state intentionally do not survive",
            self.react_native,
        )

    def test_source_comments_use_current_mobile_contract_references(self) -> None:
        """Historical HANDOFF_MOBILE references must not reappear in source docs."""
        node_source = (
            REPO_ROOT
            / "implementation"
            / "mesh-milk"
            / "crates"
            / "mmp-node"
            / "src"
            / "lib.rs"
        ).read_text(encoding="utf-8")
        mobile_source = (
            REPO_ROOT
            / "implementation"
            / "mesh-milk"
            / "crates"
            / "mmp-tal-mobile"
            / "src"
            / "lib.rs"
        ).read_text(encoding="utf-8")

        for source in (node_source, mobile_source):
            self.assertNotIn("HANDOFF_MOBILE", source)
        self.assertIn("MOBILE_INTEGRATION.md", node_source)
        self.assertIn("API_REFERENCE.md", node_source)
        self.assertIn("MOBILE_INTEGRATION.md", mobile_source)


if __name__ == "__main__":
    unittest.main()
