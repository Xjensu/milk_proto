#!/usr/bin/env bash
# Builds MmpCore.xcframework: mmp-ffi's staticlib for a real device + the
# Apple Silicon simulator, wrapped with UniFFI's generated Swift header, per
# MOBILE_INTEGRATION.md §3. macOS + Xcode only — there is no cross-platform
# way to run `xcodebuild`, so this script (like the CI job that calls it) is
# unverifiable outside a real macOS host; first real execution in CI is the
# actual verification, same honesty discipline as this repo's other
# hardware/toolchain-gated stories (see docs/AGILE_BACKLOG.md E3-S2).
#
# docs/AGILE_BACKLOG.md E2-S5.
# Usage: scripts/build-ios-xcframework.sh [output-dir]  (default: mobile-artifacts/ios)

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

OUT_DIR="${1:-mobile-artifacts/ios}"
STAGE="$OUT_DIR/.stage"
rm -rf "$STAGE" "$OUT_DIR/MmpCore.xcframework"
mkdir -p "$STAGE/headers"

echo "==> Building mmp-ffi staticlib for aarch64-apple-ios (device) and aarch64-apple-ios-sim (simulator)"
cargo build -p mmp-ffi --release --target aarch64-apple-ios
cargo build -p mmp-ffi --release --target aarch64-apple-ios-sim

echo "==> Generating Swift bindings (UniFFI) for the header + module map"
cargo run -p mmp-ffi --features cli --release --bin uniffi-bindgen -- \
    generate --library target/aarch64-apple-ios/release/libmmp_ffi.a \
    --language swift --out-dir "$STAGE/swift-bindings"

# UniFFI's swift bindgen emits the module's .swift source plus a C header
# (and on recent uniffi-bindgen versions, a .modulemap) into the same
# directory. Only the header(s) go into the xcframework; the .swift source
# is a separate consumer-side artifact (uploaded alongside, not embedded).
mkdir -p "$STAGE/swift-source"
find "$STAGE/swift-bindings" -name '*.swift' -exec mv {} "$STAGE/swift-source/" \;
find "$STAGE/swift-bindings" -name '*.h' -o -name '*.modulemap' | while read -r f; do
    cp "$f" "$STAGE/headers/"
done
if ! ls "$STAGE/headers"/*.modulemap >/dev/null 2>&1; then
    header_name=$(basename "$(ls "$STAGE/headers"/*.h | head -1)")
    module_name="${header_name%.h}"
    cat > "$STAGE/headers/module.modulemap" <<EOF
module $module_name {
    header "$header_name"
    export *
}
EOF
fi

echo "==> Assembling MmpCore.xcframework"
xcodebuild -create-xcframework \
    -library target/aarch64-apple-ios/release/libmmp_ffi.a -headers "$STAGE/headers" \
    -library target/aarch64-apple-ios-sim/release/libmmp_ffi.a -headers "$STAGE/headers" \
    -output "$OUT_DIR/MmpCore.xcframework"

mkdir -p "$OUT_DIR"
cp -r "$STAGE/swift-source" "$OUT_DIR/swift-bindings"
rm -rf "$STAGE"

echo "==> Done: $OUT_DIR/MmpCore.xcframework, $OUT_DIR/swift-bindings/"
