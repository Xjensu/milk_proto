//! Entry point for the `uniffi-bindgen` CLI binary, used to generate the
//! Kotlin/Swift bindings from the built `mmp-ffi` cdylib (see `[[bin]]` in
//! Cargo.toml, gated behind the `cli` feature). Not part of the library API;
//! never linked into mobile app builds.

fn main() {
    uniffi::uniffi_bindgen_main()
}
