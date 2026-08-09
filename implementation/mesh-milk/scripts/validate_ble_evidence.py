#!/usr/bin/env python3
"""Validate a two-host Linux/BlueZ evidence manifest.

The validator checks the evidence package, not the radio itself.  It is
deliberately independent of the Rust protocol implementation so a report can
be checked on a machine that did not run the test.  Paths in the manifest are
relative to the manifest file and may not escape its directory.

The two hosts must use distinct seed fingerprints.  Each role must also
declare the reference-CLI markers that establish its side of the exchange:
``advertising``, ``Established``, and ``MessageReceived`` for the peripheral;
``dialed``, ``Established``, and ``Delivered`` for the central.  The markers
must occur in the corresponding log.  These checks prevent a manifest from
passing with two arbitrary logs and self-selected markers, while remaining
honest that text logs are not cryptographically authenticated.

Manifest shape (``mmp-ble-evidence-v1``)::

    {
      "schema": "mmp-ble-evidence-v1",
      "run_id": "2026-08-09-linux-pair-01",
      "commit": "<40 hex characters>",
      "worktree_clean": true,
      "toolchain": {"rustc": "...", "cargo": "..."},
      "hosts": [
        {"name": "alice", "role": "peripheral", "os": "...",
         "kernel": "...", "adapter": "...", "seed_fingerprint": "...",
         "log": "alice.log", "markers": ["advertising", "MessageReceived"]},
        {"name": "bob", "role": "central", "os": "...",
         "kernel": "...", "adapter": "...", "seed_fingerprint": "...",
         "log": "bob.log", "markers": ["Established", "Delivered"]}
      ],
      "stores": {"peripheral_inspect": "alice.inspect",
                 "central_inspect": "bob.inspect"},
      "radio": {
        "mtu": {"available": true, "value": 247, "source": "btleplug"},
        "rssi_dbm": {"available": true, "samples": [-58, -55],
                      "source": "scanner callback"}
      },
      "gates": {"advertisement": true, "gatt_connection": true,
                "identity_proof": true, "bundle_transfer": true,
                "receipt_returned": true},
      "message": {"delivered": true, "bundle_id": "<32 hex characters>"},
      "receipt": {"returned": true},
      "reconnect": {"attempted": true, "stable": true, "cycles": 1}
    }

When a platform does not expose MTU or RSSI, set ``available`` to false and
provide a non-empty ``reason``.  This records the limitation instead of
silently turning an absent measurement into a pass.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


SCHEMA = "mmp-ble-evidence-v1"
COMMIT_RE = re.compile(r"^[0-9a-fA-F]{7,64}$")
ROLES = {"peripheral", "central"}
REQUIRED_GATES = (
    "advertisement",
    "gatt_connection",
    "identity_proof",
    "bundle_transfer",
    "receipt_returned",
)
# A release manifest is not allowed to reduce the Phase-A exchange to two
# arbitrary logs with self-selected markers.  These are the minimum role
# markers emitted by the reference CLI for the corresponding physical path.
# The validator still cannot authenticate a log (see the module docstring),
# but requiring both the declaration and the observed text prevents an
# incomplete/one-sided run from being presented as a complete gate.
REQUIRED_LOG_MARKERS = {
    "peripheral": ("advertising", "Established", "MessageReceived"),
    "central": ("dialed", "Established", "Delivered"),
}


def _is_mapping(value: object) -> bool:
    return isinstance(value, dict)


def _text(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _relative_file(root: Path, value: object, label: str, errors: list[str]) -> Path | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty relative path")
        return None
    candidate = Path(value)
    if candidate.is_absolute():
        errors.append(f"{label} must be relative to the manifest")
        return None
    try:
        resolved = (root / candidate).resolve()
        resolved.relative_to(root.resolve())
    except (OSError, ValueError):
        errors.append(f"{label} escapes the manifest directory")
        return None
    if not resolved.is_file():
        errors.append(f"{label} does not exist: {value}")
        return None
    return resolved


def _validate_radio(radio: object, errors: list[str]) -> None:
    if not _is_mapping(radio):
        errors.append("radio must be an object")
        return
    for key, value in (("mtu", radio.get("mtu")), ("rssi_dbm", radio.get("rssi_dbm"))):
        if not _is_mapping(value):
            errors.append(f"radio.{key} must be an object")
            continue
        available = value.get("available")
        if not isinstance(available, bool):
            errors.append(f"radio.{key}.available must be boolean")
            continue
        source = value.get("source")
        if not _text(source):
            errors.append(f"radio.{key}.source must identify where the measurement came from")
        if available:
            if key == "mtu":
                mtu = value.get("value")
                if not isinstance(mtu, int) or isinstance(mtu, bool) or mtu < 23:
                    errors.append("radio.mtu.value must be an integer >= 23 when available")
            else:
                samples = value.get("samples")
                if not isinstance(samples, list) or not samples:
                    errors.append("radio.rssi_dbm.samples must be a non-empty list when available")
                elif any(
                    isinstance(sample, bool)
                    or not isinstance(sample, (int, float))
                    or not math.isfinite(float(sample))
                    for sample in samples
                ):
                    errors.append("radio.rssi_dbm.samples must contain finite numbers")
        elif not _text(value.get("reason")):
            errors.append(f"radio.{key}.reason is required when the measurement is unavailable")


def validate_manifest(path: Path) -> list[str]:
    """Return deterministic validation errors for one evidence manifest."""

    errors: list[str] = []
    try:
        raw = path.read_text(encoding="utf-8")
        document = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        return [f"manifest cannot be read as UTF-8 JSON: {exc}"]
    if not _is_mapping(document):
        return ["manifest root must be an object"]

    if document.get("schema") != SCHEMA:
        errors.append(f"schema must be {SCHEMA!r}")
    for key in ("run_id", "commit"):
        if not _text(document.get(key)):
            errors.append(f"{key} must be a non-empty string")
    if isinstance(document.get("commit"), str) and not COMMIT_RE.fullmatch(document["commit"]):
        errors.append("commit must be a hexadecimal Git object ID (7-64 characters)")
    if document.get("worktree_clean") is not True:
        errors.append("worktree_clean must be true for a release evidence package")

    toolchain = document.get("toolchain")
    if not _is_mapping(toolchain) or not _text(toolchain.get("rustc")) or not _text(toolchain.get("cargo")):
        errors.append("toolchain.rustc and toolchain.cargo are required")

    hosts = document.get("hosts")
    if not isinstance(hosts, list) or len(hosts) != 2:
        errors.append("hosts must contain exactly two entries")
        hosts = []
    roles: set[str] = set()
    names: set[str] = set()
    seed_fingerprints: set[str] = set()
    for index, host in enumerate(hosts):
        label = f"hosts[{index}]"
        if not _is_mapping(host):
            errors.append(f"{label} must be an object")
            continue
        name = host.get("name")
        role = host.get("role")
        if not _text(name):
            errors.append(f"{label}.name must be non-empty")
        elif name in names:
            errors.append(f"duplicate host name: {name}")
        else:
            names.add(name)
        if role not in ROLES:
            errors.append(f"{label}.role must be peripheral or central")
        else:
            roles.add(role)
        for key in ("os", "kernel", "adapter", "seed_fingerprint"):
            if not _text(host.get(key)):
                errors.append(f"{label}.{key} must be recorded")
        fingerprint = host.get("seed_fingerprint")
        if _text(fingerprint):
            normalized_fingerprint = fingerprint.strip().casefold()
            if normalized_fingerprint in seed_fingerprints:
                errors.append(f"duplicate host seed_fingerprint: {fingerprint}")
            else:
                seed_fingerprints.add(normalized_fingerprint)
        log = _relative_file(path.parent, host.get("log"), f"{label}.log", errors)
        markers = host.get("markers")
        if not isinstance(markers, list) or any(not _text(marker) for marker in markers):
            errors.append(f"{label}.markers must be a list of non-empty strings")
        elif role in REQUIRED_LOG_MARKERS:
            for required_marker in REQUIRED_LOG_MARKERS[role]:
                if required_marker not in markers:
                    errors.append(f"{label}.markers must include role gate marker: {required_marker}")
        if log is not None and isinstance(markers, list):
            try:
                content = log.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as exc:
                errors.append(f"{label}.log cannot be read as UTF-8: {exc}")
            else:
                for marker in markers:
                    if isinstance(marker, str) and marker not in content:
                        errors.append(f"{label}.log is missing marker: {marker}")
    if roles != ROLES:
        errors.append("hosts must contain one peripheral and one central")

    stores = document.get("stores")
    if not _is_mapping(stores):
        errors.append("stores must be an object")
    else:
        for key in ("peripheral_inspect", "central_inspect"):
            _relative_file(path.parent, stores.get(key), f"stores.{key}", errors)

    _validate_radio(document.get("radio"), errors)

    gates = document.get("gates")
    if not _is_mapping(gates):
        errors.append("gates must be an object")
    else:
        for key in REQUIRED_GATES:
            if gates.get(key) is not True:
                errors.append(f"gates.{key} must be true")

    message = document.get("message")
    if not _is_mapping(message) or message.get("delivered") is not True:
        errors.append("message.delivered must be true")
    elif not isinstance(message.get("bundle_id"), str) or not re.fullmatch(r"[0-9a-fA-F]{32}", message["bundle_id"]):
        errors.append("message.bundle_id must be a 32-character hexadecimal BundleID")

    receipt = document.get("receipt")
    if not _is_mapping(receipt) or receipt.get("returned") is not True:
        errors.append("receipt.returned must be true")

    reconnect = document.get("reconnect")
    if not _is_mapping(reconnect):
        errors.append("reconnect must be an object")
    else:
        if reconnect.get("attempted") is not True:
            errors.append("reconnect.attempted must be true")
        if reconnect.get("stable") is not True:
            errors.append("reconnect.stable must be true")
        cycles = reconnect.get("cycles")
        if not isinstance(cycles, int) or isinstance(cycles, bool) or cycles < 1:
            errors.append("reconnect.cycles must be an integer >= 1")

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="JSON evidence manifest")
    args = parser.parse_args(argv)
    errors = validate_manifest(args.manifest)
    if errors:
        for error in errors:
            print(f"BLE evidence: {error}", file=sys.stderr)
        return 1
    print(f"BLE evidence: passed ({args.manifest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
