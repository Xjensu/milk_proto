//! mmp-cli — PC node + debugging/inspection tool (HANDOFF §2.8).
//!
//! Hand-rolled `std::env` arg parsing, no clap. `demo` runs two nodes over an
//! in-memory link so you can watch the protocol work end to end in about two
//! seconds and exits non-zero if receiver/delivery assertions fail;
//! `connect`/`send` drive a real node over BLE central role
//! (`mmp-tal-btleplug`, HANDOFF M5 / handbook §4.2); `listen` drives a real
//! node over BLE peripheral role (`mmp-tal-peripheral`, HANDOFF M5b). `keygen`
//! mints an identity seed; `inspect` reads a `mmp-store` snapshot file and
//! prints its bundle table.
//!
//! Exit codes: 0 success, 1 ran fine but the watched goal wasn't reached
//! (`send` timed out without delivery), 2 usage/setup error.

mod vectors;

use std::collections::HashSet;
use std::env;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mmp_core::store::{
    BundleMeta, BundleStore, StoreAbortError, StoreBatchEntry, StoreBatchError,
    StoreBatchReservation, StoreCommitError, StorePutError, StorePutOutcome, StoreReserveError,
};
use mmp_core::MemStore;
use mmp_node::{DeliveryState, Event, Node, Tier};
#[cfg(target_os = "windows")]
use mmp_node::WindowsVault;
use mmp_sim::energy::{
    PairedRendezvousMeasurement, RateConfidenceInterval, RendezvousComparison,
    MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES,
};
use mmp_store::SnapshotStore;
use mmp_tal::{LinkQuality, TalEvent};
use mmp_tal_btleplug::{BleCentral, PeerHandle, PeripheralId};
use mmp_tal_peripheral::PeripheralRole;
use mmp_types::ids::{Id16, NodeId};
use rand::{rngs::OsRng, RngCore};

const USAGE: &str = "\
mmp-cli — MMP/1 reference PC node

USAGE:
    mmp-cli keygen <file>                  generate a 32-byte identity seed, write hex to <file>
    mmp-cli demo [--json]                  run a two-node in-memory exchange; optionally emit a JSON report
    mmp-cli inspect <path>                 print a mmp-store snapshot's bundle table
    mmp-cli vectors [<dir>]                 (re)generate the golden test vectors into <dir>
                                            (default: the repo's vectors/ directory)
    mmp-cli edr-summary <pair.json> [--json]
                                            summarize a bounded MMP/Φ EDR measurement pair
    mmp-cli connect <seedfile> [--store <path>] [--vault <path>]
                                            central role: scan, dial newly-seen peers, drive the
                                            node over real BLE until Ctrl-C
    mmp-cli send <seedfile> <hex-nodeid> <text> [--store <path>] [--vault <path>]
                                            queue a text message, then run the same drive loop as
                                            `connect` until it is delivered or a timeout is hit
    mmp-cli listen <seedfile> [--store <path>] [--vault <path>]
                                            peripheral role: advertise, accept centrals, and drive
                                            the node over real BLE until Ctrl-C
    mmp-cli --help                         show this message

NOTE: `connect`/`send` use mmp-cli's BLE central transport (mmp-tal-btleplug)
      — it can scan and dial but cannot advertise or serve GATT. `listen` uses
      the peripheral transport (mmp-tal-peripheral) instead: peripheral role
      is Linux only today (BlueZ via `bluer`); Windows/macOS builds compile
      but cannot actually advertise yet (`PeriphError::Unsupported`) — see
      crates/mmp-tal-peripheral for status. `demo` runs the full protocol
      stack over an in-memory link, no radio required.
";

const KEYGEN_USAGE: &str = "\
mmp-cli keygen — generate an identity seed

USAGE:
    mmp-cli keygen <file>

Writes 32 bytes of OS-generated identity seed as hexadecimal text to <file>.
The command refuses to overwrite an existing file; choose a new path when
minting another identity.
The path must not begin with '-' (use ./<name> or an absolute path instead).
";

const DEMO_USAGE: &str = "\
mmp-cli demo — run a two-node in-memory exchange

USAGE:
    mmp-cli demo [--json]

With --json, stdout contains one deterministic, schema-versioned report and
no human-readable event lines. Without it, the normal diagnostic transcript is
printed unchanged.
";

const INSPECT_USAGE: &str = "\
mmp-cli inspect — print a snapshot bundle table

USAGE:
    mmp-cli inspect <path>

The snapshot path must not begin with '-'.
";

const VECTORS_USAGE: &str = "\
mmp-cli vectors — generate golden test vectors

USAGE:
    mmp-cli vectors [<dir>]

The optional output directory must not begin with '-'.
";

const EDR_SUMMARY_USAGE: &str = "\
mmp-cli edr-summary — summarize a paired EDR measurement

USAGE:
    mmp-cli edr-summary <pair.json> [--json]

The input is the strict, bounded mmp-sim paired-rendezvous JSON schema. The
command reports measurements only; it does not change MMP/1 routing, wire
fields, or platform scheduling. With --json, stdout contains a deterministic
schema-versioned summary.
The input path must not begin with '-'.
";

const CONNECT_USAGE: &str = "\
mmp-cli connect — run a BLE central node

USAGE:
    mmp-cli connect <seedfile> [--store <path>] [--vault <path>]

Both paths must not begin with '-'.
";

const SEND_USAGE: &str = "\
mmp-cli send — queue a text message and run a BLE central node

USAGE:
    mmp-cli send <seedfile> <hex-nodeid> <text> [--store <path>] [--vault <path>]

The seed and store paths must not begin with '-'.
";

const LISTEN_USAGE: &str = "\
mmp-cli listen — run a BLE peripheral node

USAGE:
    mmp-cli listen <seedfile> [--store <path>] [--vault <path>]

Both paths must not begin with '-'.
";

/// How long `send` waits for its message to reach `Delivered` before giving
/// up (exit code 1). Real DTN contacts can take a couple of round trips
/// (doc 06 §5) even over a good link, so this is generous rather than tight.
const SEND_TIMEOUT: Duration = Duration::from_secs(120);

type HostPersistenceArgs = (Vec<String>, Option<String>, Option<String>);
type BuiltNode = (Node, Option<Arc<Mutex<SnapshotStore>>>);

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            if cmd == "help" {
                cmd_help(&args[1..])
            } else if args.len() == 1 {
                print!("{USAGE}");
                ExitCode::SUCCESS
            } else {
                eprintln!("mmp-cli: help does not accept additional arguments\n");
                eprint!("{USAGE}");
                ExitCode::from(2)
            }
        }
        "keygen" => cmd_keygen(&args[1..]),
        "demo" => cmd_demo(&args[1..]),
        "inspect" => cmd_inspect(&args[1..]),
        "vectors" => cmd_vectors(&args[1..]),
        "edr-summary" => cmd_edr_summary(&args[1..]),
        "connect" => cmd_connect(&args[1..]),
        "send" => cmd_send(&args[1..]),
        "listen" => cmd_listen(&args[1..]),
        other => {
            eprintln!("mmp-cli: unknown subcommand '{other}'\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn subcommand_usage(subcmd: &str) -> &'static str {
    match subcmd {
        "keygen" => KEYGEN_USAGE,
        "demo" => DEMO_USAGE,
        "inspect" => INSPECT_USAGE,
        "vectors" => VECTORS_USAGE,
        "edr-summary" => EDR_SUMMARY_USAGE,
        "connect" => CONNECT_USAGE,
        "send" => SEND_USAGE,
        "listen" => LISTEN_USAGE,
        _ => USAGE,
    }
}

fn cmd_help(args: &[String]) -> ExitCode {
    match args {
        [] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        [arg] if is_help_arg(arg) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        [subcmd]
            if matches!(
                subcmd.as_str(),
                "keygen"
                    | "demo"
                    | "inspect"
                    | "vectors"
                    | "edr-summary"
                    | "connect"
                    | "send"
                    | "listen"
            ) =>
        {
            print!("{}", subcommand_usage(subcmd));
            ExitCode::SUCCESS
        }
        [other] => {
            eprintln!("mmp-cli: unknown subcommand '{other}'\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
        _ => {
            eprintln!("mmp-cli help: expected at most one subcommand\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn is_help_arg(arg: &str) -> bool {
    matches!(arg, "-h" | "--help" | "help")
}

/// Print a subcommand's help when its first argument is a help token. Help
/// must be the only argument so a typo cannot silently discard other input.
fn subcommand_help(subcmd: &str, args: &[String]) -> Option<ExitCode> {
    if args.first().is_some_and(|arg| is_help_arg(arg)) {
        return Some(if args.len() == 1 {
            print!("{}", subcommand_usage(subcmd));
            ExitCode::SUCCESS
        } else {
            usage_error(subcmd, "help does not accept additional arguments")
        });
    }
    None
}

fn validate_path_argument(label: &str, path: &str) -> Result<(), String> {
    if path.starts_with('-') {
        return Err(format!(
            "{label} '{path}' looks like an option; prefix it with './' or use an absolute path"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------- edr-summary --

fn cmd_edr_summary(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("edr-summary", args) {
        return result;
    }
    let (path, json) = match parse_edr_summary_args(args) {
        Ok(parsed) => parsed,
        Err(error) => return usage_error("edr-summary", &error),
    };
    let comparison = match read_edr_summary(&path) {
        Ok(comparison) => comparison,
        Err(error) => {
            eprintln!("mmp-cli edr-summary: {error}");
            return ExitCode::from(2);
        }
    };
    if json {
        println!("{}", edr_summary_json(&comparison));
    } else {
        print_edr_summary(&comparison);
    }
    ExitCode::SUCCESS
}

fn parse_edr_summary_args(args: &[String]) -> Result<(String, bool), String> {
    match args {
        [path] => {
            validate_path_argument("measurement path", path)?;
            Ok((path.clone(), false))
        }
        [path, flag] if flag == "--json" => {
            validate_path_argument("measurement path", path)?;
            Ok((path.clone(), true))
        }
        [] => Err("missing <pair.json>".to_string()),
        [_path, flag] => Err(format!("unrecognized option: {flag}")),
        _ => Err("expected <pair.json> followed by optional --json".to_string()),
    }
}

fn read_edr_summary(path: &str) -> Result<RendezvousComparison, String> {
    validate_path_argument("measurement path", path)?;
    let limit = u64::try_from(MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES)
        .expect("measurement size limit fits in u64");
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot inspect '{path}': {error}"))?;
    if metadata.len() > limit {
        return Err(format!(
            "'{path}' is too large: {} bytes (limit {limit})",
            metadata.len()
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read '{path}': {error}"))?;
    if bytes.len() > MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES {
        return Err(format!(
            "'{path}' grew beyond the {}-byte limit while it was read",
            MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES
        ));
    }
    let input = std::str::from_utf8(&bytes)
        .map_err(|_| format!("'{path}' is not valid UTF-8 JSON"))?;
    let pair = PairedRendezvousMeasurement::from_json(input)
        .map_err(|error| format!("cannot decode '{path}': {error}"))?;
    pair.summary()
        .map_err(|error| format!("cannot summarize '{path}': {error:?}"))
}

fn print_edr_summary(comparison: &RendezvousComparison) {
    println!("MMP/Φ EDR measurement summary (research-only; no protocol change)");
    println!("trace_id: {}", format_trace_id(comparison.trace_id));
    println!(
        "discovery_rate: baseline={:.6} candidate={:.6} delta={:.6}",
        comparison.baseline_discovery_rate,
        comparison.candidate_discovery_rate,
        comparison.discovery_rate_delta
    );
    println!(
        "delivery_rate: baseline={:.6} candidate={:.6} delta={:.6}",
        comparison.baseline_delivery_rate,
        comparison.candidate_delivery_rate,
        comparison.delivery_rate_delta
    );
    println!(
        "energy_mwh: radio={:.6}/{:.6} host={:.6}/{:.6} whole_device={:.6}/{:.6} candidate_over_baseline={}",
        comparison.baseline_radio_energy_mwh,
        comparison.candidate_radio_energy_mwh,
        comparison.baseline_host_energy_mwh,
        comparison.candidate_host_energy_mwh,
        comparison.baseline_whole_device_energy_mwh,
        comparison.candidate_whole_device_energy_mwh,
        format_optional(comparison.whole_device_energy_ratio)
    );
    println!(
        "latency_ms: discovery={}/{} delivery={}/{}",
        format_optional(comparison.baseline_discovery_latency_ms),
        format_optional(comparison.candidate_discovery_latency_ms),
        format_optional(comparison.baseline_delivery_latency_ms),
        format_optional(comparison.candidate_delivery_latency_ms)
    );
    println!(
        "confidence95: discovery={}/{} delivery={}/{}",
        format_interval(comparison.baseline_discovery_rate_ci95),
        format_interval(comparison.candidate_discovery_rate_ci95),
        format_interval(comparison.baseline_delivery_rate_ci95),
        format_interval(comparison.candidate_delivery_rate_ci95)
    );
}

fn edr_summary_json(comparison: &RendezvousComparison) -> String {
    let mut output = String::with_capacity(1_024);
    write!(
        output,
        "{{\"schema_version\":1,\"command\":\"edr-summary\",\"research_only\":true,\"trace_id\":\"{}\",\"baseline_discovery_rate\":{},\"candidate_discovery_rate\":{},\"discovery_rate_delta\":{},\"baseline_delivery_rate\":{},\"candidate_delivery_rate\":{},\"delivery_rate_delta\":{},\"baseline_radio_energy_mwh\":{},\"candidate_radio_energy_mwh\":{},\"baseline_host_energy_mwh\":{},\"candidate_host_energy_mwh\":{},\"baseline_whole_device_energy_mwh\":{},\"candidate_whole_device_energy_mwh\":{},\"whole_device_energy_ratio\":{}",
        format_trace_id(comparison.trace_id),
        comparison.baseline_discovery_rate,
        comparison.candidate_discovery_rate,
        comparison.discovery_rate_delta,
        comparison.baseline_delivery_rate,
        comparison.candidate_delivery_rate,
        comparison.delivery_rate_delta,
        comparison.baseline_radio_energy_mwh,
        comparison.candidate_radio_energy_mwh,
        comparison.baseline_host_energy_mwh,
        comparison.candidate_host_energy_mwh,
        comparison.baseline_whole_device_energy_mwh,
        comparison.candidate_whole_device_energy_mwh,
        format_optional_json(comparison.whole_device_energy_ratio),
    )
    .expect("writing to a String cannot fail");
    write!(
        output,
        ",\"baseline_discovery_latency_ms\":{},\"candidate_discovery_latency_ms\":{},\"baseline_delivery_latency_ms\":{},\"candidate_delivery_latency_ms\":{},\"baseline_discovery_rate_ci95\":{},\"candidate_discovery_rate_ci95\":{},\"baseline_delivery_rate_ci95\":{},\"candidate_delivery_rate_ci95\":{}}}",
        format_optional_json(comparison.baseline_discovery_latency_ms),
        format_optional_json(comparison.candidate_discovery_latency_ms),
        format_optional_json(comparison.baseline_delivery_latency_ms),
        format_optional_json(comparison.candidate_delivery_latency_ms),
        format_interval_json(comparison.baseline_discovery_rate_ci95),
        format_interval_json(comparison.candidate_discovery_rate_ci95),
        format_interval_json(comparison.baseline_delivery_rate_ci95),
        format_interval_json(comparison.candidate_delivery_rate_ci95),
    )
    .expect("writing to a String cannot fail");
    output
}

fn format_trace_id(trace_id: [u8; 16]) -> String {
    let mut output = String::with_capacity(32);
    for byte in trace_id {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn format_optional(value: Option<f64>) -> String {
    value.map_or_else(|| "undefined".to_string(), |value| format!("{value:.6}"))
}

fn format_optional_json(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn format_interval(interval: Option<RateConfidenceInterval>) -> String {
    interval.map_or_else(|| "undefined".to_string(), |value| {
        format!("[{:.6}, {:.6}]", value.lower, value.upper)
    })
}

fn format_interval_json(interval: Option<RateConfidenceInterval>) -> String {
    interval.map_or_else(|| "null".to_string(), |value| {
        format!(
            "{{\"successes\":{},\"trials\":{},\"estimate\":{},\"lower\":{},\"upper\":{}}}",
            value.successes, value.trials, value.estimate, value.lower, value.upper
        )
    })
}

// ------------------------------------------------------------------ keygen --

fn cmd_keygen(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("keygen", args) {
        return result;
    }
    let Some(path) = args.first() else {
        eprintln!("mmp-cli keygen: missing <file>\n");
        eprint!("{KEYGEN_USAGE}");
        return ExitCode::from(2);
    };
    if args.len() != 1 {
        return usage_error("keygen", "expected exactly one <file>");
    }
    if let Err(error) = validate_path_argument("output path", path) {
        return usage_error("keygen", &error);
    }

    let seed = match random_seed() {
        Ok(seed) => seed,
        Err(error) => {
            eprintln!("mmp-cli keygen: OS CSPRNG unavailable: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = write_seed_file(path, &seed) {
        eprintln!("mmp-cli keygen: failed to write '{path}': {e}");
        return ExitCode::from(2);
    }

    // Derive the node id purely to show the operator what they just made;
    // the store is never touched (Leaf tier, empty MemStore, discarded).
    let node = Node::new(seed, Box::new(MemStore::new()), Tier::Leaf);
    println!("wrote identity seed to {path}");
    println!("node id: {}", hex(node.node_id().as_bytes()));
    ExitCode::SUCCESS
}

// ----------------------------------------------------------------- vectors --

/// Regenerate the golden test vectors (E5-S4, handbook §4.7). Deterministic:
/// re-running overwrites the files with byte-identical content unless a codec
/// changed — which is exactly the review signal the consuming test asserts on.
/// The default output directory is the repo's `vectors/` (resolved from this
/// crate's manifest dir), so `mmp-cli vectors` with no argument just refreshes
/// the committed set in place.
fn cmd_vectors(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("vectors", args) {
        return result;
    }
    if args.len() > 1 {
        return usage_error("vectors", "expected at most one [<dir>]");
    }
    let default_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors");
    let dir = args.first().map(String::as_str).unwrap_or(default_dir);
    if let Err(error) = validate_path_argument("output directory", dir) {
        return usage_error("vectors", &error);
    }

    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("mmp-cli vectors: cannot create '{dir}': {e}");
        return ExitCode::from(2);
    }
    for vf in vectors::generate() {
        let path = format!("{dir}/{}", vf.name);
        if let Err(e) = std::fs::write(&path, vf.json.as_bytes()) {
            eprintln!("mmp-cli vectors: failed to write '{path}': {e}");
            return ExitCode::from(2);
        }
        println!("wrote {path}");
    }
    ExitCode::SUCCESS
}

/// Generate a 256-bit identity seed from the operating system CSPRNG.
///
/// Identity material must never be derived from clocks, process identifiers,
/// or address-space layout: those values are observable/predictable and can
/// make two independently generated identities guessable. The error is kept
/// explicit so `keygen` fails closed when the OS cannot provide entropy.
fn random_seed() -> Result<[u8; 32], rand::Error> {
    let mut seed = [0u8; 32];
    OsRng.try_fill_bytes(&mut seed)?;
    Ok(seed)
}

/// Create a seed file without ever truncating an existing identity.
///
/// Identity seeds are long-lived credentials.  An accidental second `keygen`
/// invocation must not silently replace the identity that an operator still
/// expects to own, so creation is exclusive (`create_new`) rather than the
/// usual truncating `fs::write`.  On Unix, apply the narrowest ordinary file
/// mode available before writing; Windows inherits the directory ACL because
/// the standard library has no portable ACL-construction API.  The complete
/// file is flushed before reporting success so a successful command does not
/// immediately leave a zero-length or partially buffered identity behind.
fn write_seed_file(path: &str, seed: &[u8; 32]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    std::io::Write::write_all(&mut file, hex(seed).as_bytes())?;
    file.sync_all()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Inverse of [`hex`]: decode a hex string into bytes. `None` on odd length
/// or any non-hex-digit byte — used for both seed files (`keygen`'s output)
/// and `<hex-nodeid>` arguments, so both accept exactly what `hex()` writes.
fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.as_bytes();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in s.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

/// Read and hex-decode a 32-byte identity seed file, as written by `keygen`.
fn read_seed_file(path: &str) -> Result<[u8; 32], String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read seed file '{path}': {e}"))?;
    let bytes = from_hex(contents.trim())
        .ok_or_else(|| format!("seed file '{path}' does not contain valid hex"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("seed file '{path}' must hold 32 bytes of hex (got {})", v.len()))
}

/// Decode a `<hex-nodeid>` CLI argument (32 hex chars = 16 bytes = `NodeId`).
fn parse_node_id(s: &str) -> Result<NodeId, String> {
    let bytes = from_hex(s).ok_or_else(|| format!("'{s}' is not valid hex"))?;
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("<hex-nodeid> must be 32 hex chars/16 bytes (got {})", v.len()))?;
    // `NodeId` is a type alias for `Id16` (mmp_types::ids); alias names are
    // type-namespace only, so the tuple-struct constructor must spell the
    // original name.
    Ok(Id16(arr))
}

// -------------------------------------------------------------------- demo --

fn cmd_demo(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("demo", args) {
        return result;
    }
    let json = match parse_demo_args(args) {
        Ok(json) => json,
        Err(error) => return usage_error("demo", &error),
    };
    let alice_seed = *blake3::hash(b"mmp-cli demo alice").as_bytes();
    let bob_seed = *blake3::hash(b"mmp-cli demo bob").as_bytes();
    let mut alice = Node::new(alice_seed, Box::new(MemStore::new()), Tier::Relay);
    let mut bob = Node::new(bob_seed, Box::new(MemStore::new()), Tier::Relay);
    alice.enable_diagnostics();
    bob.enable_diagnostics();

    if !json {
        println!("=== mmp-cli demo ===");
        println!("alice = {}", hex(alice.node_id().as_bytes()));
        println!("bob   = {}", hex(bob.node_id().as_bytes()));
    }

    let mut alice_events = DemoDrain::default();
    let mut bob_events = DemoDrain::default();

    // Out-of-band claim exchange (the QR / first-sync bootstrap).
    for c in alice.own_claims() {
        bob.ingest_claim(c, true);
    }
    for c in bob.own_claims() {
        alice.ingest_claim(c, true);
    }
    if json {
        alice_events.add(drain_silently(&mut alice));
        bob_events.add(drain_silently(&mut bob));
    } else {
        let _ = drain_and_print("alice", &mut alice);
        let _ = drain_and_print("bob", &mut bob);
    }

    let msg_id = alice.send_text(&bob.node_id(), "hello from mmp-cli demo").expect("bob's claims are known");
    if !json {
        println!("\nalice sends text (id {}) ...", hex(msg_id.as_bytes()));
    }

    // First contact: the message crosses the mesh.
    if !json {
        println!("\n--- contact 1 (alice <-> bob) ---");
    }
    contact(&mut alice, &mut bob, 1, 0);
    let bob_received = if json {
        let bob_drain = drain_silently(&mut bob);
        let received = bob_drain.messages_received != 0;
        bob_events.add(bob_drain);
        alice_events.add(drain_silently(&mut alice));
        received
    } else {
        let received = drain_and_print("bob", &mut bob);
        let _ = drain_and_print("alice", &mut alice);
        received
    };

    // Second contact: the delivery receipt returns to alice.
    if !json {
        println!("\n--- contact 2 (alice <-> bob) ---");
    }
    contact(&mut alice, &mut bob, 2, 1_000);
    if json {
        alice_events.add(drain_silently(&mut alice));
        bob_events.add(drain_silently(&mut bob));
    } else {
        let _ = drain_and_print("alice", &mut alice);
        let _ = drain_and_print("bob", &mut bob);
    }

    let sender_state = alice.message_state(&msg_id);
    let delivered = sender_state == Some(DeliveryState::Delivered);
    if !json {
        println!("\nmessage state at alice: {sender_state:?}");
        println!("demo assertions: receiver_message={bob_received}, sender_delivered={delivered}");
    }

    if !json {
        println!("\n--- alice diagnostics ---");
        print!("{}", alice.diagnostics().dump());
        println!("--- bob diagnostics ---");
        print!("{}", bob.diagnostics().dump());
    } else {
        print_demo_json(&DemoReport {
            alice: alice.node_id(),
            bob: bob.node_id(),
            bundle: msg_id,
            receiver_message: bob_received,
            sender_delivered: delivered,
            sender_state,
            alice_events: alice_events.events,
            bob_events: bob_events.events,
            alice_store_len: alice.store_len(),
            bob_store_len: bob.store_len(),
            alice_diagnostic_records: alice.diagnostics().records().len(),
            bob_diagnostic_records: bob.diagnostics().records().len(),
            alice_diagnostic_clamps: alice.diagnostics().clamps,
            bob_diagnostic_clamps: bob.diagnostics().clamps,
            alice_diagnostic_drops: alice.diagnostics().drops,
            bob_diagnostic_drops: bob.diagnostics().drops,
        });
    }

    if bob_received && delivered {
        ExitCode::SUCCESS
    } else {
        eprintln!("mmp-cli demo: protocol loopback did not deliver the message");
        ExitCode::from(1)
    }
}

fn parse_demo_args(args: &[String]) -> Result<bool, String> {
    match args {
        [] => Ok(false),
        [arg] if arg == "--json" => Ok(true),
        _ => Err("expected no arguments or exactly --json".to_string()),
    }
}

#[derive(Default)]
struct DemoDrain {
    events: u32,
    messages_received: u32,
}

impl DemoDrain {
    fn add(&mut self, other: Self) {
        self.events = self.events.saturating_add(other.events).min(4096);
        self.messages_received = self
            .messages_received
            .saturating_add(other.messages_received)
            .min(4096);
    }
}

fn drain_silently(n: &mut Node) -> DemoDrain {
    let mut drained = DemoDrain::default();
    while let Some(event) = n.poll_event() {
        drained.events = drained.events.saturating_add(1).min(4096);
        if matches!(event, Event::MessageReceived { .. }) {
            drained.messages_received = drained.messages_received.saturating_add(1).min(4096);
        }
    }
    drained
}

struct DemoReport {
    alice: NodeId,
    bob: NodeId,
    bundle: Id16,
    receiver_message: bool,
    sender_delivered: bool,
    sender_state: Option<DeliveryState>,
    alice_events: u32,
    bob_events: u32,
    alice_store_len: usize,
    bob_store_len: usize,
    alice_diagnostic_records: usize,
    bob_diagnostic_records: usize,
    alice_diagnostic_clamps: u64,
    bob_diagnostic_clamps: u64,
    alice_diagnostic_drops: u64,
    bob_diagnostic_drops: u64,
}

fn bounded_usize(value: usize) -> usize {
    value.min(4096)
}

fn bounded_u32(value: u32) -> u32 {
    value.min(4096)
}

fn bounded_u64(value: u64) -> u64 {
    value.min(4096)
}

fn delivery_state_name(state: Option<DeliveryState>) -> &'static str {
    match state {
        Some(DeliveryState::Queued) => "queued",
        Some(DeliveryState::InMesh) => "in_mesh",
        Some(DeliveryState::Delivered) => "delivered",
        Some(DeliveryState::Expired) => "expired",
        None => "unknown",
    }
}

fn demo_json(report: &DemoReport) -> String {
    let mut json = String::with_capacity(640);
    let _ = writeln!(
        json,
        "{{\"schema_version\":1,\"command\":\"demo\",\"alice_node_id\":\"{}\",\"bob_node_id\":\"{}\",\"bundle_id\":\"{}\",\"receiver_message\":{},\"sender_delivered\":{},\"sender_delivery_state\":\"{}\",\"contacts\":2,\"alice_events\":{},\"bob_events\":{},\"alice_store_len\":{},\"bob_store_len\":{},\"alice_diagnostic_records\":{},\"bob_diagnostic_records\":{},\"alice_diagnostic_clamps\":{},\"bob_diagnostic_clamps\":{},\"alice_diagnostic_drops\":{},\"bob_diagnostic_drops\":{}}}",
        hex(report.alice.as_bytes()),
        hex(report.bob.as_bytes()),
        hex(report.bundle.as_bytes()),
        report.receiver_message,
        report.sender_delivered,
        delivery_state_name(report.sender_state),
        bounded_u32(report.alice_events),
        bounded_u32(report.bob_events),
        bounded_usize(report.alice_store_len),
        bounded_usize(report.bob_store_len),
        bounded_usize(report.alice_diagnostic_records),
        bounded_usize(report.bob_diagnostic_records),
        bounded_u64(report.alice_diagnostic_clamps),
        bounded_u64(report.bob_diagnostic_clamps),
        bounded_u64(report.alice_diagnostic_drops),
        bounded_u64(report.bob_diagnostic_drops),
    );
    json
}

fn print_demo_json(report: &DemoReport) {
    print!("{}", demo_json(report));
}

/// Run one full contact between two nodes over an in-memory link (mirrors
/// `mmp-node`'s own `tests.rs::contact` pump pattern).
fn contact(a: &mut Node, b: &mut Node, link: u64, now: u64) {
    a.tick(now);
    b.tick(now);
    a.on_tal_event(TalEvent::LinkUp { link, initiator: true, quality: LinkQuality::default() });
    b.on_tal_event(TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() });
    for _ in 0..300 {
        let mut quiet = true;
        while let Some((l, bytes)) = a.poll_send() {
            quiet = false;
            b.on_tal_event(TalEvent::LinkData { link: l, bytes });
        }
        while let Some((l, bytes)) = b.poll_send() {
            quiet = false;
            a.on_tal_event(TalEvent::LinkData { link: l, bytes });
        }
        if quiet {
            break;
        }
    }
    a.on_tal_event(TalEvent::LinkDown { link });
    b.on_tal_event(TalEvent::LinkDown { link });
}

fn drain_and_print(who: &str, n: &mut Node) -> bool {
    let mut received = false;
    while let Some(ev) = n.poll_event() {
        if matches!(ev, Event::MessageReceived { .. }) {
            received = true;
        }
        print_event(who, &ev);
    }
    received
}

fn print_event(who: &str, ev: &Event) {
    match ev {
        Event::MessageReceived { from, body, .. } => {
            let text = String::from_utf8_lossy(body);
            println!("[{who}] MessageReceived from {}: {text:?}", hex(&from.0[..4]));
        }
        Event::Delivered { id } => {
            println!("[{who}] Delivered id={}", hex(&id.0[..4]));
        }
        other => println!("[{who}] {other:?}"),
    }
}

// ----------------------------------------------------------------- inspect --

fn cmd_inspect(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("inspect", args) {
        return result;
    }
    let Some(path) = args.first() else {
        eprintln!("mmp-cli inspect: missing <path>\n");
        eprint!("{INSPECT_USAGE}");
        return ExitCode::from(2);
    };
    if args.len() != 1 {
        return usage_error("inspect", "expected exactly one <path>");
    }
    if let Err(error) = validate_path_argument("snapshot path", path) {
        return usage_error("inspect", &error);
    }
    if !std::path::Path::new(path).exists() {
        eprintln!("mmp-cli inspect: no such file '{path}'");
        return ExitCode::from(2);
    }

    let store = SnapshotStore::open(path);
    if store.load_rejected() {
        eprintln!("mmp-cli inspect: snapshot was not loaded safely");
        return ExitCode::from(2);
    }
    let mut metas = store.all_meta();
    metas.sort_by_key(|m| m.stored_at);

    println!("{:<10} {:>5} {:>8} {:>14} {:>6}", "id", "class", "size", "expires_at_ms", "budget");
    for m in &metas {
        println!(
            "{:<10} {:>5} {:>8} {:>14} {:>6}",
            hex(&m.id.0[..4]),
            m.class,
            m.size,
            m.expires_at,
            m.budget,
        );
    }
    println!("\n{} bundle(s), {} total byte(s)", store.len(), store.total_bytes());
    ExitCode::SUCCESS
}

// ------------------------------------------------------- connect / send --
//
// `connect` and `send` share one node-hosting recipe: build a `Node` over
// real BLE central role, then run `drive_loop` (the handbook §4.2 reference
// "how to host a node" loop) until Ctrl-C (or, for `send`, until the queued
// message is delivered / times out). Only this pair of subcommands touches
// `tokio`; `main`'s other subcommands stay fully synchronous.

/// A `BundleStore` that keeps its `SnapshotStore` behind an `Arc<Mutex<_>>`
/// instead of owning it outright.
///
/// Why: `Node::new` takes `Box<dyn BundleStore + Send>` and keeps it fully
/// private — there is no way to get a `SnapshotStore` back out of a `Node`
/// to call its (non-trait) `persist()` method later. Wrapping it lets the
/// CLI keep a second `Arc` clone purely to reach `persist()` on exit, while
/// `Node` still only ever sees the store through the `BundleStore` trait
/// object it owns. This is entirely a `mmp-cli`-side concern: neither
/// `mmp-node`'s nor `mmp-store`'s public API changes.
struct SharedSnapshot(Arc<Mutex<SnapshotStore>>);

impl BundleStore for SharedSnapshot {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.0.lock().expect("snapshot store mutex poisoned").put(meta, bytes)
    }
    fn try_put(&mut self, meta: BundleMeta, bytes: &[u8]) -> Result<StorePutOutcome, StorePutError> {
        self.0.lock().expect("snapshot store mutex poisoned").try_put(meta, bytes)
    }
    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        self.0.lock().expect("snapshot store mutex poisoned").try_put_batch(entries)
    }
    fn try_reserve_batch_atomic(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<StoreBatchReservation, StoreReserveError> {
        self.0
            .lock()
            .expect("snapshot store mutex poisoned")
            .try_reserve_batch_atomic(metas)
    }
    fn commit_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError> {
        self.0
            .lock()
            .expect("snapshot store mutex poisoned")
            .commit_reserved_batch(batch, entries)
    }
    fn abort_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
    ) -> Result<(), StoreAbortError> {
        self.0
            .lock()
            .expect("snapshot store mutex poisoned")
            .abort_reserved_batch(batch)
    }
    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        self.0.lock().expect("snapshot store mutex poisoned").get(id)
    }
    fn meta(&self, id: &Id16) -> Option<BundleMeta> {
        self.0.lock().expect("snapshot store mutex poisoned").meta(id)
    }
    fn update_meta(&mut self, meta: BundleMeta) {
        self.0.lock().expect("snapshot store mutex poisoned").update_meta(meta)
    }
    fn remove(&mut self, id: &Id16) -> bool {
        self.0.lock().expect("snapshot store mutex poisoned").remove(id)
    }
    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.0.lock().expect("snapshot store mutex poisoned").ids_range(lo, hi)
    }
    fn all_meta(&self) -> Vec<BundleMeta> {
        self.0.lock().expect("snapshot store mutex poisoned").all_meta()
    }
    fn for_each_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
        self.0
            .lock()
            .expect("snapshot store mutex poisoned")
            .for_each_meta(visitor)
    }
    fn total_bytes(&self) -> u64 {
        self.0.lock().expect("snapshot store mutex poisoned").total_bytes()
    }
    fn len(&self) -> usize {
        self.0.lock().expect("snapshot store mutex poisoned").len()
    }
    fn expire(&mut self, now_ms: u64) -> Vec<Id16> {
        self.0.lock().expect("snapshot store mutex poisoned").expire(now_ms)
    }
}

/// Split `args` into exactly `n_positional` required arguments plus an
/// optional trailing `--store <path>` flag, shared by `connect` and `send`.
#[cfg(test)]
fn parse_positional_and_store(args: &[String], n_positional: usize) -> Result<(Vec<String>, Option<String>), String> {
    let (positional, store, vault) = parse_positional_store_vault(args, n_positional)?;
    if vault.is_some() {
        return Err("--vault is not accepted by this command".to_string());
    }
    Ok((positional, store))
}

/// Parse the host-only persistence options shared by the BLE commands.  The
/// two paths are intentionally separate: `--store` contains non-secret
/// protocol state, while `--vault` contains only the opaque provider journal.
fn parse_positional_store_vault(
    args: &[String],
    n_positional: usize,
) -> Result<HostPersistenceArgs, String> {
    if args.len() < n_positional {
        return Err("missing argument(s)".to_string());
    }
    let positional = args[..n_positional].to_vec();
    if let Some(seedfile) = positional.first() {
        validate_path_argument("seed file", seedfile)?;
    }
    let mut store = None;
    let mut vault = None;
    let mut rest = args[n_positional..].iter();
    while let Some(flag) = rest.next() {
        let path = rest.next().ok_or_else(|| format!("missing path after {flag}"))?;
        let (slot, label) = match flag.as_str() {
            "--store" => (&mut store, "store path"),
            "--vault" => (&mut vault, "vault path"),
            _ => return Err(format!("unrecognized option: {flag}")),
        };
        if slot.is_some() {
            return Err(format!("duplicate option: {flag}"));
        }
        validate_path_argument(label, path)?;
        *slot = Some(path.clone());
    }
    Ok((positional, store, vault))
}

#[cfg(target_os = "windows")]
type HostVault = WindowsVault;

#[cfg(not(target_os = "windows"))]
struct HostVault;

fn open_host_vault(path: Option<&str>) -> Result<Option<HostVault>, String> {
    #[cfg(target_os = "windows")]
    {
        path.map(|path| WindowsVault::open(path).map_err(|error| format!("cannot open Windows vault: {error:?}")))
            .transpose()
    }
    #[cfg(not(target_os = "windows"))]
    {
        if path.is_some() {
            Err("--vault is supported only by the Windows desktop provider".to_string())
        } else {
            Ok(None)
        }
    }
}

fn restore_host_vault(node: &mut Node, vault: &mut Option<HostVault>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(vault) = vault {
            node.restore_secret_state_from_windows_vault(vault)
                .map(|generation| {
                    if let Some(generation) = generation {
                        println!("secret state restored at generation {generation}");
                    }
                })
                .map_err(|error| format!("cannot restore Windows vault: {error:?}"))?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (node, vault);
    Ok(())
}

fn save_host_vault(node: &mut Node, vault: &mut Option<HostVault>) -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Some(vault) = vault {
            return match node.save_secret_state_to_windows_vault(vault) {
                Ok(generation) => {
                    println!("secret state persisted at generation {generation}");
                    true
                }
                Err(error) => {
                    eprintln!("mmp-cli: failed to persist Windows vault: {error:?}");
                    false
                }
            };
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (node, vault);
    true
}

fn usage_error(subcmd: &str, msg: &str) -> ExitCode {
    eprintln!("mmp-cli {subcmd}: {msg}\n");
    eprint!("{}", subcommand_usage(subcmd));
    ExitCode::from(2)
}

/// Build a `Tier::Anchor` node over `seed`, backed by a `SnapshotStore` at
/// `store_path` if given, else an in-memory `MemStore` (same store type
/// `demo` uses, discarded on exit). Returns the shared handle to persist
/// through later, if a `SnapshotStore` is in play — see [`SharedSnapshot`].
/// A snapshot that failed closed during loading is rejected before a `Node`
/// exists, so the CLI cannot process traffic against a RAM-only replacement.
fn build_node(
    seed: [u8; 32],
    store_path: Option<&str>,
) -> Result<BuiltNode, String> {
    match store_path {
        Some(path) => {
            let shared = Arc::new(Mutex::new(SnapshotStore::open(path)));
            if shared.lock().expect("snapshot store mutex poisoned").load_rejected() {
                return Err(format!(
                    "snapshot '{path}' was not loaded safely and cannot be replaced"
                ));
            }
            let mut node = Node::new(seed, Box::new(SharedSnapshot(shared.clone())), Tier::Anchor);
            restore_snapshot_state(&mut node, &shared);
            Ok((node, Some(shared)))
        }
        None => Ok((Node::new(seed, Box::new(MemStore::new()), Tier::Anchor), None)),
    }
}

/// Restore the non-secret node state carried by [`SnapshotStore`] before the
/// host starts processing traffic.  The FFI constructor follows the same
/// ordering: claims/groups/filters are loaded after `Node::new` (so the node
/// identity and fresh local claims already exist) but before any BLE event is
/// accepted.  A persisted claim is not an out-of-band trust decision, so it
/// is ingested with `verified: false`; the store has already re-verified its
/// signature while loading it.
fn restore_snapshot_state(node: &mut Node, shared: &Arc<Mutex<SnapshotStore>>) {
    // Copy the validated snapshot records while holding the store lock, then
    // release it before mutating the node.  `Node` is backed by the same
    // `SharedSnapshot`; ingest/restore paths may touch that store again and
    // must never attempt to reacquire this mutex recursively.
    let (claims, group_blobs, seen_index, receipt_filter, authority, blocklist) = {
        let store = shared.lock().expect("snapshot store mutex poisoned");
        (
            store.claims().to_vec(),
            store.group_blobs().to_vec(),
            store.seen_index().cloned(),
            store.receipt_filter().cloned(),
            store.authority().map(ToOwned::to_owned),
            store.blocklist().map(ToOwned::to_owned),
        )
    };

    for claim in &claims {
        node.ingest_claim(claim.clone(), false);
    }
    for blob in &group_blobs {
        node.restore_group(blob);
    }
    if let Some(seen) = seen_index {
        node.restore_seen_index(seen);
    }
    if let Some(receipts) = receipt_filter {
        node.restore_receipt_filter(receipts);
    }
    if let Some(authority) = authority {
        let _ = node.restore_authority(&authority);
    }
    if let Some(blocklist) = blocklist {
        let _ = node.restore_blocklist(&blocklist);
    }
}

/// Stage the complete non-secret node state and atomically persist the shared
/// snapshot. Calling `SnapshotStore::persist` alone only writes the bundle
/// store; all receiver-local admission/replay state is explicitly staged by
/// the host just before the write. Identity and ratchet secrets never enter
/// this file-backed snapshot.
fn persist_node_state(shared: &Arc<Mutex<SnapshotStore>>, node: &Node) -> std::io::Result<()> {
    let (claims, group_blobs, seen_index, receipt_filter, authority, blocklist) = (
        node.all_claims(),
        node.all_group_blobs(),
        node.seen_index(),
        node.receipt_filter(),
        node.authority_snapshot(),
        node.blocklist_snapshot(),
    );
    let mut store = shared.lock().expect("snapshot store mutex poisoned");
    store.set_claims(claims);
    store.set_group_blobs(group_blobs);
    store.set_seen_index(seen_index);
    store.set_receipt_filter(receipt_filter);
    store.set_authority(authority);
    store.set_blocklist(blocklist);
    store.persist()
}

/// Best-effort wall-clock source used by the standalone CLI.
///
/// The node API calls its injected value `now_ms` and uses it as a monotonic
/// process timebase. The CLI also needs persisted snapshot deadlines to age
/// across a restart, so it intentionally supplies Unix milliseconds here.
/// This adapter does not clamp, rebase, or infer reboot offsets; those policies
/// belong to the host that owns the clock (the mobile FFI contract makes this
/// explicit). Keeping the source behind a seam makes that limitation visible
/// and lets tests exercise call ordering without a real clock.
trait ClockSource {
    fn now_ms(&mut self) -> u64;
}

struct WallClock;

impl ClockSource for WallClock {
    fn now_ms(&mut self) -> u64 {
        wall_clock_ms()
    }
}

fn wall_clock_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Bring a freshly restored node onto the CLI's persisted deadline timebase
/// before any command creates new bundles. `Node::new` starts at zero, while
/// the current snapshot format stores absolute millisecond deadlines; without
/// this first tick, a `send` bundle created before the drive loop's first tick
/// would expire immediately when the loop supplies the Unix timestamp.
fn prime_node_clock<C: ClockSource>(node: &mut Node, clock: &mut C) {
    node.tick(clock.now_ms());
}

/// Finalize host state when the async runtime cannot be created.  `send`
/// queues a message before starting the runtime, so this path can carry new
/// ratchet/secret state even though no BLE drive loop ever starts.  Keep the
/// same fail-closed persistence boundary as every later loop exit.
fn runtime_start_failure(
    subcmd: &str,
    error: &dyn std::fmt::Display,
    shared: Option<&Arc<Mutex<SnapshotStore>>>,
    node: &mut Node,
    vault: &mut Option<HostVault>,
) -> ExitCode {
    eprintln!("mmp-cli {subcmd}: failed to start async runtime: {error}");
    let _ = finish(shared, node, vault, 0, 0);
    ExitCode::from(2)
}

fn cmd_connect(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("connect", args) {
        return result;
    }
    let (positional, store_path, vault_path) = match parse_positional_store_vault(args, 1) {
        Ok(v) => v,
        Err(e) => return usage_error("connect", &e),
    };
    let seed = match read_seed_file(&positional[0]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mmp-cli connect: {e}");
            return ExitCode::from(2);
        }
    };

    let (mut node, shared) = match build_node(seed, store_path.as_deref()) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("mmp-cli connect: {error}");
            return ExitCode::from(2);
        }
    };
    let mut vault = match open_host_vault(vault_path.as_deref()) {
        Ok(vault) => vault,
        Err(error) => {
            eprintln!("mmp-cli connect: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = restore_host_vault(&mut node, &mut vault) {
        eprintln!("mmp-cli connect: {error}");
        return ExitCode::from(2);
    }
    let mut clock = WallClock;
    prime_node_clock(&mut node, &mut clock);
    println!("node id: {}", hex(node.node_id().as_bytes()));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return runtime_start_failure("connect", &e, shared.as_ref(), &mut node, &mut vault),
    };
    rt.block_on(drive_loop(&mut node, shared.as_ref(), &mut vault, &mut clock, None))
}

fn cmd_send(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("send", args) {
        return result;
    }
    let (positional, store_path, vault_path) = match parse_positional_store_vault(args, 3) {
        Ok(v) => v,
        Err(e) => return usage_error("send", &e),
    };
    let seed = match read_seed_file(&positional[0]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mmp-cli send: {e}");
            return ExitCode::from(2);
        }
    };
    let dest = match parse_node_id(&positional[1]) {
        Ok(id) => id,
        Err(e) => return usage_error("send", &e),
    };

    let (mut node, shared) = match build_node(seed, store_path.as_deref()) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("mmp-cli send: {error}");
            return ExitCode::from(2);
        }
    };
    let mut vault = match open_host_vault(vault_path.as_deref()) {
        Ok(vault) => vault,
        Err(error) => {
            eprintln!("mmp-cli send: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = restore_host_vault(&mut node, &mut vault) {
        eprintln!("mmp-cli send: {error}");
        return ExitCode::from(2);
    }
    let mut clock = WallClock;
    prime_node_clock(&mut node, &mut clock);
    println!("node id: {}", hex(node.node_id().as_bytes()));

    let msg_id = match node.send_text(&dest, &positional[2]) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("mmp-cli send: cannot queue message: {e:?}");
            eprintln!(
                "  (the destination's claims are not yet known — run `connect` first so the two \
                 nodes exchange claims, or persist a `--store` from a prior contact)"
            );
            return ExitCode::from(2);
        }
    };
    println!("queued message {} to {}", hex(msg_id.as_bytes()), hex(dest.as_bytes()));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return runtime_start_failure("send", &e, shared.as_ref(), &mut node, &mut vault),
    };
    rt.block_on(drive_loop(&mut node, shared.as_ref(), &mut vault, &mut clock, Some(msg_id)))
}

fn cmd_listen(args: &[String]) -> ExitCode {
    if let Some(result) = subcommand_help("listen", args) {
        return result;
    }
    let (positional, store_path, vault_path) = match parse_positional_store_vault(args, 1) {
        Ok(v) => v,
        Err(e) => return usage_error("listen", &e),
    };
    let seed = match read_seed_file(&positional[0]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mmp-cli listen: {e}");
            return ExitCode::from(2);
        }
    };

    let (mut node, shared) = match build_node(seed, store_path.as_deref()) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("mmp-cli listen: {error}");
            return ExitCode::from(2);
        }
    };
    let mut vault = match open_host_vault(vault_path.as_deref()) {
        Ok(vault) => vault,
        Err(error) => {
            eprintln!("mmp-cli listen: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(error) = restore_host_vault(&mut node, &mut vault) {
        eprintln!("mmp-cli listen: {error}");
        return ExitCode::from(2);
    }
    let mut clock = WallClock;
    prime_node_clock(&mut node, &mut clock);
    println!("node id: {}", hex(node.node_id().as_bytes()));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return runtime_start_failure("listen", &e, shared.as_ref(), &mut node, &mut vault),
    };
    rt.block_on(drive_loop_peripheral(&mut node, shared.as_ref(), &mut vault, &mut clock))
}

/// THE reference "how to host a node" loop (handbook §4.2), central role.
///
/// Every ~1 s: `tick`. On every BLE event: hand it to `on_tal_event` (this
/// includes `TalEvent::Beacon` — `mmp-tal-btleplug` only shuttles bytes, it
/// is `mmp-node` that decodes the beacon and notes the ephemeral id). After
/// any event/tick: drain transport-local `poll_close` actions, then `poll_send`
/// onto the matching BLE link, and drain `poll_event` to print. `mmp-cli`'s BLE transport is central-only (module
/// docs), so this loop never calls `Node::beacon`/advertises — that is the
/// peripheral role's job, driven by `listen`/`drive_loop_peripheral` (HANDOFF
/// M5b).
///
/// Dial policy (the handbook leaves this open): dial every peer this
/// process has seen beacon from, once. A beacon's payload cannot be
/// correlated to the `PeripheralId` that sent it (this crate forwards raw
/// bytes without parsing — see `mmp-tal-btleplug`'s module docs), so on
/// every `Beacon` event we re-list `known_peers()` and dial anyone not yet
/// in the `dialed` de-dupe set. One attempt per peer for the process's
/// lifetime (no retry/backoff) — simple and sufficient for a reference CLI
/// talking to a handful of nearby devices; a production node would want
/// RSSI-aware prioritization and retry (doc 05 §6).
///
/// `watch`, set only by `send`, is the message id to wait for: the loop
/// returns as soon as `Event::Delivered { id }` matches it, or after
/// `SEND_TIMEOUT` with no such event (exit code 1, see module docs).
async fn drive_loop<C: ClockSource>(
    node: &mut Node,
    shared: Option<&Arc<Mutex<SnapshotStore>>>,
    vault: &mut Option<HostVault>,
    clock: &mut C,
    watch: Option<Id16>,
) -> ExitCode {
    let central = match BleCentral::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mmp-cli: no usable BLE adapter: {e}");
            // A host may have advanced timers or restored local state before
            // the adapter was opened.  Persist that state even on setup
            // failure, and fail closed if the snapshot cannot be written.
            let _ = finish(shared, node, vault, 0, 0);
            return ExitCode::from(2);
        }
    };
    if let Err(e) = central.start_scan().await {
        eprintln!("mmp-cli: failed to start BLE scan: {e}");
        central.shutdown_all().await;
        let _ = finish(shared, node, vault, 0, 0);
        return ExitCode::from(2);
    }
    println!("scanning for MMP peers... (Ctrl-C to stop)");

    let mut events = central.events();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut dialed: HashSet<PeripheralId> = HashSet::new();
    let start = std::time::Instant::now();

    let mut frames_sent = 0u64;
    let mut messages_received = 0u64;
    let mut transport_ended = false;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                node.tick(clock.now_ms());
            }
            ev = events.recv() => {
                match ev {
                    Some(TalEvent::Beacon { payload, rssi }) => {
                        dial_new_peers(&central, &mut dialed).await;
                        node.on_tal_event(TalEvent::Beacon { payload, rssi });
                    }
                    Some(other) => node.on_tal_event(other),
                    // The central's background task ended (adapter gone) —
                    // nothing more will ever arrive; wind down like Ctrl-C.
                    None => {
                        eprintln!("mmp-cli: BLE event stream ended unexpectedly");
                        transport_ended = true;
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nCtrl-C received, shutting down...");
                break;
            }
        }

        while let Some(link) = node.poll_close() {
            central.hangup(link).await;
        }
        if node.close_actions_dropped() != 0
            || node.event_queue_dropped() != 0
            || central.control_events_dropped() != 0
        {
            eprintln!(
                "mmp-cli: transport lifecycle overload (node close drops: {}, node event drops: {}, transport control drops: {}); shutting down",
                node.close_actions_dropped(),
                node.event_queue_dropped(),
                central.control_events_dropped()
            );
            central.shutdown_all().await;
            finish(shared, node, vault, frames_sent, messages_received);
            return ExitCode::from(2);
        }
        while let Some((link, bytes)) = node.poll_send() {
            if let Err(e) = central.send(link, &bytes).await {
                eprintln!("mmp-cli: send on link {link:#x} failed: {e}");
            } else {
                frames_sent += 1;
            }
        }
        while let Some(event) = node.poll_event() {
            print_event("node", &event);
            match &event {
                Event::MessageReceived { .. } => messages_received += 1,
                Event::Delivered { id } if watch == Some(*id) => {
                    println!("message {} delivered", hex(id.as_bytes()));
                    central.shutdown_all().await;
                    return if finish(shared, node, vault, frames_sent, messages_received) {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(2)
                    };
                }
                _ => {}
            }
        }

        if watch.is_some() && start.elapsed() >= SEND_TIMEOUT {
            println!("timed out after {}s waiting for delivery", SEND_TIMEOUT.as_secs());
            central.shutdown_all().await;
            return if finish(shared, node, vault, frames_sent, messages_received) {
                ExitCode::from(1)
            } else {
                ExitCode::from(2)
            };
        }
    }

    let overload = node.close_actions_dropped() != 0
        || node.event_queue_dropped() != 0
        || central.control_events_dropped() != 0;
    central.shutdown_all().await;
    let persisted = finish(shared, node, vault, frames_sent, messages_received);
    drive_exit_code(transport_ended, overload, persisted)
}

/// Dial every beacon-sighted peer not already in `dialed`. See `drive_loop`
/// docs for the policy rationale.
async fn dial_new_peers(central: &BleCentral, dialed: &mut HashSet<PeripheralId>) {
    let peers: Vec<PeerHandle> = match central.known_peers().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("mmp-cli: could not list known peers: {e}");
            return;
        }
    };
    for peer in peers {
        if !dialed.insert(peer.id.clone()) {
            continue; // already attempted this peer this run
        }
        let id = peer.id.clone();
        match central.dial(peer).await {
            Ok(link) => println!("dialed {id:?} -> link {link:#x}"),
            Err(e) => eprintln!("mmp-cli: dial {id:?} failed: {e}"),
        }
    }
}

/// THE reference "how to host a node" loop (handbook §4.2), peripheral role.
///
/// Mirrors `drive_loop` (central role) but the event source is
/// [`PeripheralRole::events`] instead of `BleCentral::events`, and outbound
/// bytes go through [`PeripheralRole::notify`] instead of `central.send`.
/// Advertising is this loop's job (the central-role loop never advertises —
/// see `drive_loop`'s docs): it advertises once up front and again whenever
/// [`mmp_tal_peripheral::readvertise_due`] says the current beacon has rolled
/// into a new `EPHEM_SLOT_MS` slot, keeping the on-air beacon in step with
/// what `Node::beacon` would mint for "now" (doc 09 §3.2). `listen` never
/// watches for a specific delivery (that is `send`'s job over the central
/// role), so this loop always runs until Ctrl-C or the peripheral's event
/// channel closes.
async fn drive_loop_peripheral<C: ClockSource>(
    node: &mut Node,
    shared: Option<&Arc<Mutex<SnapshotStore>>>,
    vault: &mut Option<HostVault>,
    clock: &mut C,
) -> ExitCode {
    let peripheral = match PeripheralRole::new().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("mmp-cli: no usable BLE peripheral backend: {e}");
            let _ = finish(shared, node, vault, 0, 0);
            return ExitCode::from(2);
        }
    };

    let mut last_advertised_ms = clock.now_ms();
    if let Err(e) = peripheral.advertise(node.beacon(last_advertised_ms).to_vec()).await {
        eprintln!("mmp-cli: failed to start advertising: {e}");
        peripheral.shutdown_all().await;
        let _ = finish(shared, node, vault, 0, 0);
        return ExitCode::from(2);
    }
    println!("advertising for MMP peers... (Ctrl-C to stop)");

    let mut events = peripheral.events();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    let mut frames_sent = 0u64;
    let mut messages_received = 0u64;
    let mut transport_ended = false;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let now = clock.now_ms();
                node.tick(now);
                if mmp_tal_peripheral::readvertise_due(last_advertised_ms, now) {
                    match peripheral.advertise(node.beacon(now).to_vec()).await {
                        Ok(()) => last_advertised_ms = now,
                        Err(e) => eprintln!("mmp-cli: re-advertise failed: {e}"),
                    }
                }
            }
            ev = events.recv() => {
                match ev {
                    Some(event) => node.on_tal_event(event),
                    // The peripheral's background task ended (adapter gone) —
                    // nothing more will ever arrive; wind down like Ctrl-C.
                    None => {
                        eprintln!("mmp-cli: BLE event stream ended unexpectedly");
                        transport_ended = true;
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nCtrl-C received, shutting down...");
                break;
            }
        }

        while let Some(link) = node.poll_close() {
            if let Err(e) = peripheral.close_link(link).await {
                eprintln!("mmp-cli: close on link {link:#x} failed: {e}");
            }
        }
        if node.close_actions_dropped() != 0
            || node.event_queue_dropped() != 0
            || peripheral.control_events_dropped() != 0
        {
            eprintln!(
                "mmp-cli: transport lifecycle overload (node close drops: {}, node event drops: {}, transport control drops: {}); shutting down",
                node.close_actions_dropped(),
                node.event_queue_dropped(),
                peripheral.control_events_dropped()
            );
            peripheral.shutdown_all().await;
            finish(shared, node, vault, frames_sent, messages_received);
            return ExitCode::from(2);
        }
        while let Some((link, bytes)) = node.poll_send() {
            if let Err(e) = peripheral.notify(link, &bytes).await {
                eprintln!("mmp-cli: notify on link {link:#x} failed: {e}");
            } else {
                frames_sent += 1;
            }
        }
        while let Some(event) = node.poll_event() {
            print_event("node", &event);
            if let Event::MessageReceived { .. } = &event {
                messages_received += 1;
            }
        }
    }

    let overload = node.close_actions_dropped() != 0
        || node.event_queue_dropped() != 0
        || peripheral.control_events_dropped() != 0;
    peripheral.shutdown_all().await;
    let persisted = finish(shared, node, vault, frames_sent, messages_received);
    drive_exit_code(transport_ended, overload, persisted)
}

/// Convert abnormal drive-loop termination into a fail-closed process exit.
/// A closed transport event stream is not equivalent to Ctrl-C: it means the
/// adapter/backend stopped delivering lifecycle events, so a beta harness must
/// not mistake the persisted snapshot for a healthy run.
fn drive_exit_code(transport_ended: bool, overload: bool, persisted: bool) -> ExitCode {
    if transport_ended || overload || !persisted { ExitCode::from(2) } else { ExitCode::SUCCESS }
}

/// Exit path shared by every `drive_loop` return: persist to disk if a
/// `--store` path is in play, then print the run summary. A failed persistence
/// operation is returned to the caller so a host cannot report a successful
/// run while silently losing its restart state.
fn finish(
    shared: Option<&Arc<Mutex<SnapshotStore>>>,
    node: &mut Node,
    vault: &mut Option<HostVault>,
    frames_sent: u64,
    messages_received: u64,
) -> bool {
    let persisted = match shared {
        Some(shared) => match persist_node_state(shared, node) {
            Ok(()) => {
                println!("store persisted to disk");
                true
            }
            Err(e) => {
                eprintln!("mmp-cli: failed to persist store: {e}");
                false
            }
        },
        None => true,
    };
    let vault_persisted = save_host_vault(node, vault);
    println!("--- summary ---");
    println!("frames sent to links:  {frames_sent}");
    println!("messages received:     {messages_received}");
    println!("store_len:             {}", node.store_len());
    persisted && vault_persisted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use mmp_core::store::SlotState;
    use mmp_types::bundle::class;
    use mmp_types::ids::Key32;

    struct SequenceClock(VecDeque<u64>);

    impl SequenceClock {
        fn new(values: impl IntoIterator<Item = u64>) -> Self {
            Self(values.into_iter().collect())
        }
    }

    impl ClockSource for SequenceClock {
        fn now_ms(&mut self) -> u64 {
            self.0.pop_front().expect("test clock exhausted")
        }
    }

    fn temp_snapshot_path() -> std::path::PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("mmp-cli-snapshot-{}-{nonce}.bin", std::process::id()))
    }

    fn temp_seed_path() -> std::path::PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("mmp-cli-seed-{}-{nonce}.hex", std::process::id()))
    }

    fn remove_snapshot(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("tmp"));
    }

    #[test]
    fn primed_cli_clock_keeps_a_new_bundle_alive_on_first_drive_tick() {
        let seed = [0x31; 32];
        let peer = Node::new([0x32; 32], Box::new(MemStore::new()), Tier::Anchor);
        let mut node = Node::new(seed, Box::new(MemStore::new()), Tier::Anchor);
        for claim in peer.own_claims() {
            node.ingest_claim(claim, false);
        }

        let mut clock = SequenceClock::new([1_000_000, 1_000_001]);
        prime_node_clock(&mut node, &mut clock);
        node.send_text(&peer.node_id(), "clock-safe").expect("peer claims are present");
        assert!(node.store_len() > 0);
        node.tick(clock.now_ms());
        assert!(node.store_len() > 0, "the first drive tick must not expire a freshly queued bundle");
    }

    #[test]
    fn demo_smoke_requires_receiver_and_delivery_evidence() {
        assert_eq!(cmd_demo(&[]), ExitCode::SUCCESS);
    }

    #[test]
    fn demo_argument_parser_accepts_only_explicit_json_mode() {
        assert_eq!(parse_demo_args(&[]), Ok(false));
        assert_eq!(parse_demo_args(&["--json".to_string()]), Ok(true));
        assert!(parse_demo_args(&["--json".to_string(), "extra".to_string()]).is_err());
        assert!(parse_demo_args(&["--json=true".to_string()]).is_err());
    }

    #[test]
    fn demo_json_report_has_stable_schema_and_bounded_public_fields() {
        let report = DemoReport {
            alice: Id16([0x11; 16]),
            bob: Id16([0x22; 16]),
            bundle: Id16([0x33; 16]),
            receiver_message: true,
            sender_delivered: true,
            sender_state: Some(DeliveryState::Delivered),
            alice_events: u32::MAX,
            bob_events: u32::MAX,
            alice_store_len: usize::MAX,
            bob_store_len: usize::MAX,
            alice_diagnostic_records: usize::MAX,
            bob_diagnostic_records: usize::MAX,
            alice_diagnostic_clamps: u64::MAX,
            bob_diagnostic_clamps: u64::MAX,
            alice_diagnostic_drops: u64::MAX,
            bob_diagnostic_drops: u64::MAX,
        };
        let json = demo_json(&report);
        assert!(json.ends_with('\n'));
        assert!(json.starts_with("{\"schema_version\":1,\"command\":\"demo\""));
        assert!(json.contains("\"alice_node_id\":\"11111111111111111111111111111111\""));
        assert!(json.contains("\"sender_delivery_state\":\"delivered\""));
        assert!(json.contains("\"alice_events\":4096"));
        assert!(json.contains("\"alice_store_len\":4096"));
        assert!(!json.contains("seed"));
        assert!(!json.contains("ciphertext"));
    }

    #[test]
    fn keygen_help_is_handled_before_any_output_path_operation() {
        let help = vec!["--help".to_string()];
        assert_eq!(cmd_keygen(&help), ExitCode::SUCCESS);
        assert!(subcommand_usage("keygen").contains("mmp-cli keygen <file>"));
    }

    #[test]
    fn seed_creation_is_exclusive_and_does_not_overwrite_an_identity() {
        let path = temp_seed_path();
        let _ = std::fs::remove_file(&path);
        let path_string = path.to_string_lossy().into_owned();
        let first = [0x11; 32];
        let second = [0x22; 32];

        write_seed_file(&path_string, &first).expect("first seed creation must succeed");
        let original = std::fs::read_to_string(&path).expect("seed must be readable");
        let error = write_seed_file(&path_string, &second).expect_err("existing seed must not be replaced");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);

        let _ = std::fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn seed_creation_uses_owner_only_permissions_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_seed_path();
        let _ = std::fs::remove_file(&path);
        let path_string = path.to_string_lossy().into_owned();
        write_seed_file(&path_string, &[0x33; 32]).expect("seed creation must succeed");

        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn every_documented_subcommand_has_explicit_help() {
        for subcmd in [
            "keygen",
            "demo",
            "inspect",
            "vectors",
            "edr-summary",
            "connect",
            "send",
            "listen",
        ] {
            let text = subcommand_usage(subcmd);
            assert!(text.contains(&format!("mmp-cli {subcmd}")), "missing help for {subcmd}");
        }
        assert_eq!(cmd_help(&["keygen".to_string()]), ExitCode::SUCCESS);
    }

    #[test]
    fn edr_summary_parser_is_strict_and_rejects_option_looking_paths() {
        assert_eq!(parse_edr_summary_args(&[]), Err("missing <pair.json>".to_string()));
        assert_eq!(
            parse_edr_summary_args(&["trace.json".to_string()]),
            Ok(("trace.json".to_string(), false))
        );
        assert_eq!(
            parse_edr_summary_args(&["trace.json".to_string(), "--json".to_string()]),
            Ok(("trace.json".to_string(), true))
        );
        assert!(parse_edr_summary_args(&["--help".to_string()]).is_err());
        assert!(parse_edr_summary_args(&[
            "trace.json".to_string(),
            "--json=true".to_string()
        ])
        .is_err());
        assert!(parse_edr_summary_args(&["trace.json".to_string(), "extra".to_string()]).is_err());
    }

    #[test]
    fn edr_summary_reads_bounded_pair_and_emits_stable_json() {
        use mmp_sim::energy::{PairedRendezvousMeasurement, RendezvousMeasurement};

        let trace_id = [0x4a; 16];
        let baseline = RendezvousMeasurement {
            trace_id,
            tx_events: 10,
            rx_windows: 5,
            contact_attempts: 10,
            discoveries: 4,
            offered_bundles: 4,
            delivered_bundles: 2,
            useful_payload_bytes: 512,
            airtime_ms: 10.0,
            discovery_latency_ms_sum: 40.0,
            discovery_latency_samples: 4,
            delivery_latency_ms_sum: 20.0,
            delivery_latency_samples: 2,
            radio_energy_mwh: 4.0,
            host_energy_mwh: 2.0,
            whole_device_energy_mwh: 8.0,
            host_wakeups: 5,
        };
        let candidate = RendezvousMeasurement {
            discoveries: 5,
            delivered_bundles: 3,
            radio_energy_mwh: 3.0,
            host_energy_mwh: 1.5,
            whole_device_energy_mwh: 6.0,
            discovery_latency_ms_sum: 30.0,
            delivery_latency_ms_sum: 18.0,
            ..baseline
        };
        let pair = PairedRendezvousMeasurement::new(baseline, candidate).unwrap();
        let path = env::temp_dir().join(format!("mmp-cli-edr-{}.json", std::process::id()));
        let path_string = path.to_string_lossy().into_owned();
        std::fs::write(&path, pair.to_json().unwrap()).unwrap();

        let comparison = read_edr_summary(&path_string).unwrap();
        assert_eq!(comparison.trace_id, trace_id);
        assert_eq!(comparison.whole_device_energy_ratio, Some(0.75));
        let json = edr_summary_json(&comparison);
        assert!(json.starts_with("{\"schema_version\":1,\"command\":\"edr-summary\""));
        assert!(json.contains("\"research_only\":true"));
        assert!(json.contains("\"trace_id\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\""));
        assert!(!json.contains("seed"));
        assert!(!json.contains("ciphertext"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn edr_summary_rejects_oversized_input_before_json_decode() {
        let path = env::temp_dir().join(format!("mmp-cli-edr-large-{}.json", std::process::id()));
        let path_string = path.to_string_lossy().into_owned();
        std::fs::write(
            &path,
            vec![b' '; MAX_PAIRED_RENDEZVOUS_MEASUREMENT_JSON_BYTES + 1],
        )
        .unwrap();
        let error = read_edr_summary(&path_string).expect_err("oversized input must fail closed");
        assert!(error.contains("too large"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn option_looking_paths_are_rejected_but_message_text_remains_free_form() {
        assert!(validate_path_argument("output path", "--help").is_err());
        let store = vec!["seed.hex", "00112233445566778899aabbccddeeff", "--help"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert!(parse_positional_and_store(&store, 3).is_ok());

        let bad_store = vec!["seed.hex", "--store", "--help"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert!(parse_positional_and_store(&bad_store, 1)
            .expect_err("option-looking store paths must fail")
            .contains("looks like an option"));
    }

    #[test]
    fn host_persistence_options_are_parsed_separately() {
        let args = [
            "seed.hex",
            "--store",
            "state.mmp",
            "--vault",
            "secret.mmpv",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let (positional, store, vault) = parse_positional_store_vault(&args, 1).unwrap();
        assert_eq!(positional, vec!["seed.hex"]);
        assert_eq!(store.as_deref(), Some("state.mmp"));
        assert_eq!(vault.as_deref(), Some("secret.mmpv"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cli_vault_save_on_exit_round_trips_before_host_clock() {
        let path = env::temp_dir().join(format!("mmp-cli-vault-{}.bin", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let path_string = path.to_string_lossy().into_owned();
        let seed = [0x71; 32];

        let (mut source, _) = build_node(seed, None).expect("in-memory node construction");
        let mut vault = open_host_vault(Some(&path_string)).unwrap();
        restore_host_vault(&mut source, &mut vault).unwrap();
        assert!(
            finish(None, &mut source, &mut vault, 0, 0),
            "CLI exit must persist the Windows vault before reporting success"
        );
        drop(vault);

        let (mut restored, _) = build_node(seed, None).expect("in-memory node construction");
        let mut reopened = open_host_vault(Some(&path_string)).unwrap();
        restore_host_vault(&mut restored, &mut reopened).unwrap();
        assert_eq!(restored.node_id(), source.node_id());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("bin.lock"));
        let _ = std::fs::remove_file(path.with_extension("tmp"));
    }

    #[test]
    fn shared_snapshot_preserves_bounded_try_put_rejection() {
        let path = temp_snapshot_path();
        let snapshot = SnapshotStore::open_with_limits(&path, mmp_store::SnapshotLimits::new(1));
        let mut shared = SharedSnapshot(Arc::new(Mutex::new(snapshot)));
        let meta = BundleMeta {
            id: Id16([0x61; 16]),
            class: class::DIRECT,
            origin: Id16([0x62; 16]),
            dest: Id16([0x63; 16]),
            priority: 1,
            effective_priority: 1,
            size: 0,
            stored_at: 0,
            expires_at: 1_000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };

        assert_eq!(shared.try_put(meta, &[]), Err(StorePutError::Capacity));
        assert_eq!(shared.len(), 0);
        remove_snapshot(&path);
    }

    #[test]
    fn shared_snapshot_forwards_atomic_batch_outcomes() {
        let path = temp_snapshot_path();
        let snapshot = SnapshotStore::open(&path);
        let mut shared = SharedSnapshot(Arc::new(Mutex::new(snapshot)));
        let meta = |id_byte: u8, size: usize| BundleMeta {
            id: Id16([id_byte; 16]),
            class: class::DIRECT,
            origin: Id16([0x72; 16]),
            dest: Id16([0x73; 16]),
            priority: 1,
            effective_priority: 1,
            size: size as u32,
            stored_at: 0,
            expires_at: 1_000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };
        let entries = [
            StoreBatchEntry::new(meta(0x74, 3), b"one"),
            StoreBatchEntry::new(meta(0x75, 3), b"two"),
        ];

        assert_eq!(
            shared.try_put_batch(&entries),
            Ok(vec![StorePutOutcome::Inserted, StorePutOutcome::Inserted])
        );
        assert_eq!(shared.len(), 2);
        assert_eq!(shared.get(&Id16([0x74; 16])).map(|(_, bytes)| bytes), Some(b"one".to_vec()));
        assert_eq!(shared.get(&Id16([0x75; 16])).map(|(_, bytes)| bytes), Some(b"two".to_vec()));
        remove_snapshot(&path);
    }

    #[test]
    fn shared_snapshot_forwards_atomic_reservation_commit_and_records() {
        let path = temp_snapshot_path();
        let snapshot = SnapshotStore::open(&path);
        let mut shared = SharedSnapshot(Arc::new(Mutex::new(snapshot)));
        let meta = |id_byte: u8, size: usize| BundleMeta {
            id: Id16([id_byte; 16]),
            class: class::DIRECT,
            origin: Id16([0x76; 16]),
            dest: Id16([0x77; 16]),
            priority: 1,
            effective_priority: 1,
            size: size as u32,
            stored_at: 0,
            expires_at: 1_000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };
        let metas = [meta(0x78, 3), meta(0x79, 3)];
        let entries = [
            StoreBatchEntry::new(metas[0], b"one"),
            StoreBatchEntry::new(metas[1], b"two"),
        ];

        let batch = shared
            .try_reserve_batch_atomic(&metas)
            .expect("snapshot capability must be forwarded");
        assert_eq!(
            shared
                .commit_reserved_batch(&batch, &entries)
                .expect("reserved batch must commit through adapter"),
            vec![StorePutOutcome::Inserted, StorePutOutcome::Inserted]
        );
        assert_eq!(shared.len(), 2);
        assert_eq!(
            shared.get(&metas[0].id).map(|(_, bytes)| bytes),
            Some(b"one".to_vec())
        );
        assert_eq!(
            shared.get(&metas[1].id).map(|(_, bytes)| bytes),
            Some(b"two".to_vec())
        );
        assert_eq!(
            shared.abort_reserved_batch(&batch),
            Ok(()),
            "aborting a consumed reservation must be forwarded idempotently"
        );
        assert_eq!(shared.len(), 2, "abort must not remove committed records");
        remove_snapshot(&path);
    }

    #[test]
    fn shared_snapshot_forwards_atomic_reservation_abort_without_record() {
        let path = temp_snapshot_path();
        let snapshot = SnapshotStore::open(&path);
        let mut shared = SharedSnapshot(Arc::new(Mutex::new(snapshot)));
        let meta = BundleMeta {
            id: Id16([0x7a; 16]),
            class: class::DIRECT,
            origin: Id16([0x7b; 16]),
            dest: Id16([0x7c; 16]),
            priority: 1,
            effective_priority: 1,
            size: 3,
            stored_at: 0,
            expires_at: 1_000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };

        let batch = shared
            .try_reserve_batch_atomic(&[meta])
            .expect("snapshot capability must be forwarded");
        assert_eq!(shared.len(), 0);
        assert_eq!(shared.abort_reserved_batch(&batch), Ok(()));
        assert_eq!(shared.abort_reserved_batch(&batch), Ok(()));
        assert_eq!(shared.meta(&meta.id), None);
        assert_eq!(shared.len(), 0, "aborting must not publish a record");
        remove_snapshot(&path);
    }

    #[test]
    fn cli_clock_seam_forwards_raw_samples_without_skew_policy() {
        let mut clock = SequenceClock::new([1_000, 1_001, 999]);
        assert_eq!(clock.now_ms(), 1_000);
        assert_eq!(clock.now_ms(), 1_001);
        assert_eq!(clock.now_ms(), 999);
    }

    #[test]
    fn ended_transport_fails_closed_even_when_state_persisted() {
        assert_eq!(drive_exit_code(true, false, true), ExitCode::from(2));
        assert_eq!(drive_exit_code(false, false, true), ExitCode::SUCCESS);
        assert_eq!(drive_exit_code(false, true, true), ExitCode::from(2));
        assert_eq!(drive_exit_code(false, false, false), ExitCode::from(2));
    }

    #[test]
    fn finish_fails_closed_when_snapshot_cannot_be_written() {
        let missing_parent = env::temp_dir().join(format!(
            "mmp-cli-missing-parent-{}-{}",
            std::process::id(),
            wall_clock_ms()
        ));
        let path = missing_parent.join("state.bin");
        let shared = Arc::new(Mutex::new(SnapshotStore::open(&path)));
        let node = Node::new([0x41; 32], Box::new(MemStore::new()), Tier::Anchor);

        let mut node = node;
        let mut vault = None;
        assert!(
            !finish(Some(&shared), &mut node, &mut vault, 0, 0),
            "a successful CLI exit must never be reported when persistence failed"
        );
        assert!(!path.exists());
        assert!(!missing_parent.exists());
    }

    #[test]
    fn runtime_start_failure_persists_before_fail_closed_exit() {
        let path = temp_snapshot_path();
        let shared = Arc::new(Mutex::new(SnapshotStore::open(&path)));
        let mut node = Node::new([0x42; 32], Box::new(MemStore::new()), Tier::Anchor);
        let mut vault = None;
        let error = std::io::Error::other("test runtime failure");

        assert_eq!(
            runtime_start_failure("send", &error, Some(&shared), &mut node, &mut vault),
            ExitCode::from(2)
        );
        assert!(path.exists(), "runtime startup failure must still finalize state");
        assert!(!SnapshotStore::open(&path).load_rejected());

        remove_snapshot(&path);
    }

    #[test]
    fn build_node_rejects_unreadable_snapshot_before_hosting() {
        let path = temp_snapshot_path();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("directory primary simulates an unreadable snapshot");
        let path_string = path.to_string_lossy().into_owned();

        assert!(
            build_node([0x52; 32], Some(&path_string)).is_err(),
            "a rejected snapshot must not produce a live RAM-only node"
        );
        assert!(path.is_dir(), "the rejected primary remains available for repair");

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn inspect_rejects_unreadable_snapshot_instead_of_reporting_empty() {
        let path = temp_snapshot_path();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("directory primary simulates an unreadable snapshot");
        let path_string = path.to_string_lossy().into_owned();

        assert_eq!(cmd_inspect(&[path_string]), ExitCode::from(2));

        let _ = std::fs::remove_dir_all(path);
    }

    /// The CLI must perform the same bootstrap sequence as the FFI host:
    /// restored claims are usable for a send immediately, group membership is
    /// available before the first contact, receiver-local authority admission
    /// and block decisions are retained, and replay/receipt filters survive
    /// the restart. This also exercises the exit-time staging helper rather
    /// than calling `SnapshotStore::persist` directly.
    #[test]
    fn snapshot_bootstrap_restores_claims_groups_and_filters() {
        let path = temp_snapshot_path();
        let path_string = path.to_string_lossy().into_owned();
        let seed = [0x11; 32];
        let peer_seed = [0x22; 32];
        let peer = Node::new(peer_seed, Box::new(MemStore::new()), Tier::Anchor);

        let (mut node, shared) = build_node(seed, Some(&path_string)).expect("valid snapshot path");
        for claim in peer.own_claims() {
            node.ingest_claim(claim, false);
        }
        assert!(node.send_text(&peer.node_id(), "before restart").is_ok());
        node.set_nickname("restored-cli", 2);
        let gid = node.create_group();

        let seen_id = Id16([0x44; 16]);
        let mut seen = node.seen_index();
        seen.insert(&seen_id, class::DIRECT, 1);
        node.restore_seen_index(seen);

        let receipt_id = Id16([0x55; 16]);
        let mut receipts = node.receipt_filter();
        receipts.note_verified_receipt(&receipt_id, 1);
        node.restore_receipt_filter(receipts);

        let authority_scope = Id16([0x66; 16]);
        node.pin_authority_root(authority_scope, Key32([0x77; 32]));
        node.subscribe_scope_id(authority_scope);
        node.block(peer.node_id());
        let authority_snapshot = node.authority_snapshot();
        let blocklist_snapshot = node.blocklist_snapshot();

        persist_node_state(shared.as_ref().expect("file-backed snapshot"), &node).expect("persist node state");
        drop(shared);
        drop(node);

        let (mut restored, reopened) = build_node(seed, Some(&path_string)).expect("reopen snapshot path");
        assert!(restored.send_text(&peer.node_id(), "after restart").is_ok());
        assert_eq!(restored.nickname_of(&restored.node_id()).as_deref(), Some("restored-cli"));
        assert_eq!(restored.group_members(&gid), vec![restored.node_id()]);
        assert!(restored.seen_index().contains(&seen_id));
        assert_eq!(restored.receipt_filter().signal(&receipt_id), mmp_core::SuppressSignal::Purge);
        assert_eq!(restored.authority_snapshot(), authority_snapshot);
        assert_eq!(restored.blocklist_snapshot(), blocklist_snapshot);
        assert!(reopened.is_some());

        remove_snapshot(&path);
    }
}
