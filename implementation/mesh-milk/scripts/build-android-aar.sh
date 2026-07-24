#!/usr/bin/env bash
# Builds mmp-ffi.aar: a native-only Android AAR wrapping mmp-ffi's cdylib for
# every ABI in MOBILE_INTEGRATION.md §3's cargo-ndk invocation, plus the
# UniFFI-generated Kotlin bindings as a source zip alongside it.
#
# docs/AGILE_BACKLOG.md E2-S5. Prerequisites (not installed by this script):
#   - rustup targets: aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android
#   - cargo-ndk (`cargo install cargo-ndk`)
#   - ANDROID_NDK_HOME pointing at an installed NDK
#
# A "native-only" AAR (manifest + jni/<abi>/*.so, no compiled classes.jar) is
# a standard, documented Android Gradle Plugin dependency shape — AGP merges
# an AAR's jni/ folder into the consuming app's jniLibs automatically. This
# repo does not yet have an Android app module (that's E4-S1, explicitly out
# of scope for a Rust-only session) to compile the generated Kotlin bindings
# into classes.jar, so the bindings ship as source (mmp-ffi-kotlin-bindings.zip,
# same content the already-shipped `mobile-bindings` CI job produces) for a
# consuming app to compile alongside its own code — not baked into the AAR.
#
# Usage: scripts/build-android-aar.sh [output-dir]  (default: mobile-artifacts/android)

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

OUT_DIR="${1:-mobile-artifacts/android}"
STAGE="$OUT_DIR/.stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/jni/arm64-v8a" "$STAGE/jni/armeabi-v7a" "$STAGE/jni/x86_64"

echo "==> Building mmp-ffi cdylib for arm64-v8a / armeabi-v7a / x86_64 (cargo-ndk)"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$STAGE/jni-raw" build -p mmp-ffi --release

# cargo-ndk lays out $STAGE/jni-raw/<abi>/libmmp_ffi.so; move into the AAR's
# expected jni/<abi>/ layout (AAR ABI dir names match cargo-ndk's already).
for abi in arm64-v8a armeabi-v7a x86_64; do
    cp "$STAGE/jni-raw/$abi/libmmp_ffi.so" "$STAGE/jni/$abi/libmmp_ffi.so"
done
rm -rf "$STAGE/jni-raw"

echo "==> Writing minimal AndroidManifest.xml (native-only AAR, no components)"
cat > "$STAGE/AndroidManifest.xml" <<'EOF'
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="org.meshmilk.ffi">
</manifest>
EOF

echo "==> Generating Kotlin bindings (UniFFI, same pipeline as CI's mobile-bindings job)"
cargo build -p mmp-ffi --release
cargo run -p mmp-ffi --features cli --release --bin uniffi-bindgen -- \
    generate --library target/release/libmmp_ffi.so --language kotlin --out-dir "$STAGE/kotlin-bindings"

mkdir -p "$OUT_DIR"
( cd "$STAGE" && zip -rq "../mmp-ffi.aar" AndroidManifest.xml jni )
( cd "$STAGE/kotlin-bindings" && zip -rq "../../mmp-ffi-kotlin-bindings.zip" . )
rm -rf "$STAGE"

echo "==> Done: $OUT_DIR/mmp-ffi.aar, $OUT_DIR/mmp-ffi-kotlin-bindings.zip"
