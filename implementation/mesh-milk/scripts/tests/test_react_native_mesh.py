import json
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
PACKAGE = ROOT / "platform" / "react-native-mesh"
CLIENT = PACKAGE / "src" / "MmpClient.ts"
TURBO = PACKAGE / "src" / "NativeMmpMesh.ts"
README = PACKAGE / "README.md"


class ReactNativeMeshSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.client = CLIENT.read_text(encoding="utf-8")
        cls.turbo = TURBO.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")
        cls.package = json.loads((PACKAGE / "package.json").read_text(encoding="utf-8"))

    def test_package_metadata_has_only_react_peers(self) -> None:
        self.assertEqual(set(self.package["peerDependencies"]), {"react", "react-native"})
        self.assertNotIn("dependencies", self.package)
        self.assertEqual(self.package["main"], "dist/index.js")
        self.assertEqual(self.package["types"], "dist/index.d.ts")
        self.assertIn("build", self.package["scripts"])

    def test_client_matches_documented_method_inventory(self) -> None:
        methods = (
            "start",
            "stop",
            "setTier",
            "nodeId",
            "publicKey",
            "ownClaims",
            "ingestClaim",
            "setNickname",
            "revokeIdentity",
            "tryRevokeIdentity",
            "revokeIdentityWithSuccessor",
            "block",
            "sendText",
            "messageState",
            "createGroup",
            "groupInvite",
            "groupSend",
            "groupMembers",
            "groupRemove",
            "enableDiagnostics",
            "diagDump",
            "storeLen",
            "flush",
            "lastPersistError",
            "onEvent",
        )
        for method in methods:
            self.assertIn(f"{method}(", self.client)
        self.assertIn("export const mmp = new MmpClient()", self.client)

    def test_encoding_and_event_shapes_are_explicit(self) -> None:
        for alias in ("NodeIdHex", "MsgIdHex", "GroupIdHex"):
            self.assertIn(f"export type {alias} = string", self.client)
        for event_type in (
            "messageReceived",
            "messageGap",
            "delivered",
            "deliveryProgress",
            "claimUpdated",
            "compromiseWarning",
            "peerRevoked",
            "contactSeen",
            "established",
            "groupInvite",
            "groupMessage",
            "groupEpochChanged",
            "tierChanged",
        ):
            self.assertIn(f"type: '{event_type}'", self.client)
        self.assertIn("claimB64: string", self.client)
        self.assertIn("successorPublicKeyB64: string", self.client)
        self.assertIn("countersigB64: string", self.client)

    def test_missing_native_module_is_rejected_before_calls(self) -> None:
        self.assertIn("MmpNativeModuleError", self.client)
        self.assertIn("MMP native module MmpMesh is unavailable", self.client)
        self.assertIn("MMP native module MmpMesh is incomplete", self.client)
        self.assertIn("requireMmpNativeModule(nativeModule)", self.client)
        self.assertIn("constructor(nativeModule?: unknown", self.client)

    def test_default_singleton_import_is_lazy_but_injected_module_is_checked(self) -> None:
        self.assertIn("export const mmp = new MmpClient()", self.client)
        self.assertIn("this.usesDefaultNative = arguments.length === 0 || nativeModule === undefined", self.client)
        self.assertIn("if (!this.usesDefaultNative)", self.client)
        self.assertIn("this.resolvedNative = requireMmpNativeModule(this.usesDefaultNative ? undefined : this.nativeSource)", self.client)
        self.assertIn("The exported default client is lazy", self.readme)

    def test_raw_ble_bytes_do_not_cross_the_js_surface(self) -> None:
        lowered = self.client.lower()
        for forbidden in ("linkdata", "linkup", "linkdown", "pollsend", "outframe"):
            self.assertNotIn(forbidden, lowered)
        self.assertIn("raw BLE frames never cross", self.client)
        self.assertIn("never be sent through the React Native JS bridge", self.readme)

    def test_turbo_module_surface_is_documented(self) -> None:
        for method in (
            "start",
            "stop",
            "setTier",
            "nodeId",
            "publicKey",
            "sendText",
            "messageState",
            "createGroup",
            "groupInvite",
            "groupSend",
            "groupMembers",
            "groupRemove",
            "setNickname",
            "revokeIdentity",
            "tryRevokeIdentity",
            "revokeIdentityWithSuccessor",
            "ownClaims",
            "ingestClaim",
            "block",
            "enableDiagnostics",
            "diagDump",
            "storeLen",
            "flush",
            "lastPersistError",
            "addListener",
            "removeListeners",
        ):
            self.assertIn(f"{method}(", self.turbo)
        self.assertIn("getEnforcing<Spec>('MmpMesh')", self.turbo)


if __name__ == "__main__":
    unittest.main()
