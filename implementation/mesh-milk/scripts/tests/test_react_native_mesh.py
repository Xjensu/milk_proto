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
            "status",
            "metrics",
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
            "sendFile",
            "fetchFile",
            "cancelFetch",
            "fileProgress",
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
            "fileOffered",
            "fileReady",
            "fileFailed",
        ):
            self.assertIn(f"type: '{event_type}'", self.client)
        self.assertIn("claimB64: string", self.client)
        self.assertIn("successorPublicKeyB64: string", self.client)
        self.assertIn("countersigB64: string", self.client)
        self.assertIn("export type FileRootHex = string", self.client)
        self.assertIn("export type Base64Value = string", self.client)
        self.assertIn("export interface FileSend", self.client)
        self.assertIn("messageId: MsgIdHex", self.client)
        self.assertIn("export interface FileProgress", self.client)
        self.assertIn("haveSymbols: number", self.client)
        self.assertIn("export interface FileOffer", self.client)
        self.assertIn("nameB64: Base64Value", self.client)
        self.assertIn("mimeB64: Base64Value", self.client)
        self.assertIn("thumbRoot: FileRootHex | null", self.client)
        self.assertIn("export interface NodeStatus", self.client)
        self.assertIn("persistentStore: boolean", self.client)
        self.assertIn("diagnosticsEnabled: boolean", self.client)
        self.assertIn("lastTickMs: number", self.client)
        self.assertIn("export interface NodeMetrics", self.client)
        for field in (
            "storeLen",
            "closeActionsDropped",
            "eventQueueDropped",
            "diagnosticRecords",
            "diagnosticClamps",
            "diagnosticDrops",
        ):
            self.assertIn(f"{field}: number", self.client)

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
            "status",
            "metrics",
            "nodeId",
            "publicKey",
            "sendText",
            "messageState",
            "sendFile",
            "fetchFile",
            "cancelFetch",
            "fileProgress",
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

    def test_beta_snapshots_are_secret_free_and_exported(self) -> None:
        self.assertIn("status(): Promise<NodeStatus>", self.client)
        self.assertIn("metrics(): Promise<NodeMetrics>", self.client)
        self.assertIn("status(): Promise<NodeStatus>", self.turbo)
        self.assertIn("metrics(): Promise<NodeMetrics>", self.turbo)
        self.assertNotIn("from './MmpClient'", self.turbo)
        self.assertIn("sendFile(peer: string, name: string, mime: string, dataB64: string)", self.turbo)
        self.assertIn("fetchFile(root: string, dest: string)", self.turbo)
        self.assertIn("cancelFetch(root: string)", self.turbo)
        self.assertIn("fileProgress(root: string)", self.turbo)
        self.assertIn("type NodeStatus = {", self.turbo)
        self.assertIn("type NodeMetrics = {", self.turbo)
        for field in (
            "tier: string",
            "persistentStore: boolean",
            "diagnosticsEnabled: boolean",
            "lastTickMs: number",
            "storeLen: number",
            "closeActionsDropped: number",
            "eventQueueDropped: number",
            "diagnosticRecords: number",
            "diagnosticClamps: number",
            "diagnosticDrops: number",
        ):
            self.assertIn(field, self.turbo)
        index = (PACKAGE / "src" / "index.ts").read_text(encoding="utf-8")
        self.assertIn("NodeMetrics", index)
        self.assertIn("NodeStatus", index)
        lowered = self.client.lower()
        for forbidden in ("providerhandle", "peerid", "sessionid", "wirebytes"):
            self.assertNotIn(forbidden, lowered)

    def test_file_transfer_is_explicit_pull_and_secret_free(self) -> None:
        self.assertIn("dataB64: Base64Value", self.client)
        self.assertIn("fileOffered", self.readme)
        self.assertIn("explicitly pull", self.readme)
        self.assertIn("fileReady", self.readme)
        self.assertIn("fileFailed", self.readme)
        self.assertNotIn("privateKey", self.client)

    def test_file_failure_enum_mapping_is_explicit(self) -> None:
        self.assertIn("export type NativeFileFailReason = 'HashMismatch' | 'Abandoned'", self.client)
        self.assertIn("mapNativeFileFailReason", self.client)
        self.assertIn("case 'HashMismatch':", self.client)
        self.assertIn("return 'hashMismatch';", self.client)
        self.assertIn("case 'Abandoned':", self.client)
        self.assertIn("return 'abandoned';", self.client)
        self.assertNotIn("value.toLowerCase()", self.client)
        self.assertNotIn("value.lowercase()", self.client)
        self.assertIn("Generic lower-casing is not valid", self.readme)
        self.assertIn("older `DeliveryState` casing contract is unchanged", self.readme)


if __name__ == "__main__":
    unittest.main()
