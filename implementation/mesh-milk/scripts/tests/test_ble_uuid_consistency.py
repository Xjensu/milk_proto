import re
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
TAL = ROOT / "crates" / "mmp-tal" / "src" / "lib.rs"
KOTLIN = (
    ROOT
    / "platform"
    / "kotlin-mesh-drive"
    / "src"
    / "main"
    / "kotlin"
    / "org"
    / "mesh_milk"
    / "platform"
    / "MeshDrive.kt"
)
SWIFT = ROOT / "platform" / "swift-mesh-drive" / "Sources" / "MeshDrive.swift"
MOBILE = ROOT / "MOBILE_INTEGRATION.md"


def rust_const(source: str, name: str) -> str:
    match = re.search(rf'pub const {name}: &str = "([^"]+)";', source)
    if match is None:
        raise AssertionError(f"missing Rust constant {name}")
    return match.group(1)


class BleUuidConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tal = TAL.read_text(encoding="utf-8")
        cls.kotlin = KOTLIN.read_text(encoding="utf-8")
        cls.swift = SWIFT.read_text(encoding="utf-8")
        cls.mobile = MOBILE.read_text(encoding="utf-8")

    def test_native_shells_and_manual_contract_match_rust_transport_constants(self) -> None:
        service = rust_const(self.tal, "DEV_SERVICE_UUID")
        tx = rust_const(self.tal, "DEV_TX_CHARACTERISTIC_UUID")
        rx = rust_const(self.tal, "DEV_RX_CHARACTERISTIC_UUID")

        self.assertIn(f'SERVICE_UUID: String = "{service}"', self.kotlin)
        self.assertIn(f'serviceUUID = "{service}"', self.swift)
        self.assertIn(f"* Service: `{service}`", self.mobile)
        self.assertIn(f"* TX (client→server, Write Without Response): `{tx}`", self.mobile)
        self.assertIn(f"* RX (server→client, Notify): `{rx}`", self.mobile)


if __name__ == "__main__":
    unittest.main()
