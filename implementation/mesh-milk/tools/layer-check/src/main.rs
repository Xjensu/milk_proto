//! Layer-dependency test (doc 12 §1, ROADMAP Stage 0).
//!
//! Rule: nothing below `mmp-node` may depend on tokio, std time, or any TAL
//! implementation. "Below mmp-node" = the protocol-logic layers plus the
//! seam/trait crates: mmp-types, mmp-crypto, mmp-core, mmp-router,
//! mmp-rs-code, mmp-store, mmp-tal.
//!
//! Two checks, both transitive where it matters:
//! 1. `cargo tree -p <crate>` output must not mention any banned package
//!    (tokio, the three TAL implementation crates). `cargo tree` resolves
//!    transitively, so an indirect tokio dependency is caught too.
//! 2. Source scan: the lower-layer crates' `src/` must not reference
//!    `std::time` or `tokio::` in code. (For the #![no_std] four this is
//!    belt-and-braces — the attribute already forbids std — but mmp-store
//!    and mmp-tal are std-capable and need the scan.)
//!
//! Exit code 0 = rule holds; 1 = violation (printed).

use std::path::{Path, PathBuf};
use std::process::Command;

const LOWER_LAYERS: &[&str] = &[
    "mmp-types",
    "mmp-crypto",
    "mmp-core",
    "mmp-router",
    "mmp-rs-code",
    "mmp-store",
    "mmp-tal",
];

const BANNED_PACKAGES: &[&str] = &[
    "tokio",
    "mmp-tal-btleplug",
    "mmp-tal-mobile",
    "mmp-tal-embedded",
    "mmp-tal-peripheral",
];

const BANNED_SOURCE_TOKENS: &[&str] = &["std::time", "tokio::"];

fn workspace_root() -> PathBuf {
    // layer-check runs via `cargo run -p layer-check` from anywhere in the
    // workspace; CARGO_MANIFEST_DIR is <root>/tools/layer-check.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/layer-check sits two levels below the workspace root")
        .to_path_buf()
}

fn check_dependency_graph(root: &Path, failures: &mut Vec<String>) {
    for krate in LOWER_LAYERS {
        let out = Command::new(env!("CARGO"))
            .current_dir(root)
            .args(["tree", "-p", krate, "-e", "normal", "--prefix", "none"])
            .output()
            .expect("cargo tree must run");
        if !out.status.success() {
            failures.push(format!(
                "cargo tree -p {krate} failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
            continue;
        }
        let tree = String::from_utf8_lossy(&out.stdout);
        for line in tree.lines() {
            let pkg = line.split_whitespace().next().unwrap_or("");
            if BANNED_PACKAGES.contains(&pkg) {
                failures.push(format!(
                    "layer violation: {krate} (transitively) depends on banned package `{pkg}`"
                ));
            }
        }
    }
}

fn scan_sources(dir: &Path, krate: &str, failures: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_sources(&path, krate, failures);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for token in BANNED_SOURCE_TOKENS {
                if text.contains(token) {
                    failures.push(format!(
                        "layer violation: {krate}: `{token}` referenced in {}",
                        path.display()
                    ));
                }
            }
        }
    }
}

fn main() {
    let root = workspace_root();
    let mut failures = Vec::new();

    check_dependency_graph(&root, &mut failures);
    for krate in LOWER_LAYERS {
        scan_sources(&root.join("crates").join(krate).join("src"), krate, &mut failures);
    }

    if failures.is_empty() {
        println!(
            "layer-check OK: {} lower-layer crates clean of {:?} and {:?}",
            LOWER_LAYERS.len(),
            BANNED_PACKAGES,
            BANNED_SOURCE_TOKENS
        );
    } else {
        for f in &failures {
            eprintln!("{f}");
        }
        std::process::exit(1);
    }
}
