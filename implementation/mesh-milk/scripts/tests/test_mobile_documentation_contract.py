import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class MobileDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.api_reference = (REPO_ROOT / "docs" / "API_REFERENCE.md").read_text(
            encoding="utf-8"
        )
        cls.handbook = (REPO_ROOT / "docs" / "IMPLEMENTATION_HANDBOOK.md").read_text(
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
        self.assertIn("try_revoke_identity(reason) -> Result<Vec<u8>, FfiError>", self.handbook)
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


if __name__ == "__main__":
    unittest.main()
