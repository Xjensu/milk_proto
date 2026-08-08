//! mmp-cli — PC node + debugging/inspection tool (HANDOFF §2.8).
//!
//! Hand-rolled `std::env` arg parsing, no clap. `demo` runs two nodes over an
//! in-memory link so you can watch the protocol work end to end in about two
//! seconds; `connect`/`send` drive a real node over BLE central role
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
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mmp_core::store::{BundleMeta, BundleStore};
use mmp_core::MemStore;
use mmp_node::{Event, Node, Tier};
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
    mmp-cli demo                           run a two-node in-memory exchange and print what happens
    mmp-cli inspect <path>                 print a mmp-store snapshot's bundle table
    mmp-cli vectors [<dir>]                 (re)generate the golden test vectors into <dir>
                                            (default: the repo's vectors/ directory)
    mmp-cli connect <seedfile> [--store <path>]
                                            central role: scan, dial newly-seen peers, drive the
                                            node over real BLE until Ctrl-C
    mmp-cli send <seedfile> <hex-nodeid> <text> [--store <path>]
                                            queue a text message, then run the same drive loop as
                                            `connect` until it is delivered or a timeout is hit
    mmp-cli listen <seedfile> [--store <path>]
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

/// How long `send` waits for its message to reach `Delivered` before giving
/// up (exit code 1). Real DTN contacts can take a couple of round trips
/// (doc 06 §5) even over a good link, so this is generous rather than tight.
const SEND_TIMEOUT: Duration = Duration::from_secs(120);

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "keygen" => cmd_keygen(&args[1..]),
        "demo" => cmd_demo(),
        "inspect" => cmd_inspect(&args[1..]),
        "vectors" => cmd_vectors(&args[1..]),
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

// ------------------------------------------------------------------ keygen --

fn cmd_keygen(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("mmp-cli keygen: missing <file>\n");
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };

    let seed = match random_seed() {
        Ok(seed) => seed,
        Err(error) => {
            eprintln!("mmp-cli keygen: OS CSPRNG unavailable: {error}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = std::fs::write(path, hex(&seed)) {
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
    let default_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vectors");
    let dir = args.first().map(String::as_str).unwrap_or(default_dir);

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

fn cmd_demo() -> ExitCode {
    let alice_seed = *blake3::hash(b"mmp-cli demo alice").as_bytes();
    let bob_seed = *blake3::hash(b"mmp-cli demo bob").as_bytes();
    let mut alice = Node::new(alice_seed, Box::new(MemStore::new()), Tier::Relay);
    let mut bob = Node::new(bob_seed, Box::new(MemStore::new()), Tier::Relay);
    alice.enable_diagnostics();
    bob.enable_diagnostics();

    println!("=== mmp-cli demo ===");
    println!("alice = {}", hex(alice.node_id().as_bytes()));
    println!("bob   = {}", hex(bob.node_id().as_bytes()));

    // Out-of-band claim exchange (the QR / first-sync bootstrap).
    for c in alice.own_claims() {
        bob.ingest_claim(c, true);
    }
    for c in bob.own_claims() {
        alice.ingest_claim(c, true);
    }
    drain_and_print("alice", &mut alice);
    drain_and_print("bob", &mut bob);

    let msg_id = alice.send_text(&bob.node_id(), "hello from mmp-cli demo").expect("bob's claims are known");
    println!("\nalice sends text (id {}) ...", hex(msg_id.as_bytes()));

    // First contact: the message crosses the mesh.
    println!("\n--- contact 1 (alice <-> bob) ---");
    contact(&mut alice, &mut bob, 1, 0);
    drain_and_print("alice", &mut alice);
    drain_and_print("bob", &mut bob);

    // Second contact: the delivery receipt returns to alice.
    println!("\n--- contact 2 (alice <-> bob) ---");
    contact(&mut alice, &mut bob, 2, 1_000);
    drain_and_print("alice", &mut alice);
    drain_and_print("bob", &mut bob);

    println!("\nmessage state at alice: {:?}", alice.message_state(&msg_id));

    println!("\n--- alice diagnostics ---");
    print!("{}", alice.diagnostics().dump());
    println!("--- bob diagnostics ---");
    print!("{}", bob.diagnostics().dump());

    ExitCode::SUCCESS
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

fn drain_and_print(who: &str, n: &mut Node) {
    while let Some(ev) = n.poll_event() {
        print_event(who, &ev);
    }
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
    let Some(path) = args.first() else {
        eprintln!("mmp-cli inspect: missing <path>\n");
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    if !std::path::Path::new(path).exists() {
        eprintln!("mmp-cli inspect: no such file '{path}'");
        return ExitCode::from(2);
    }

    let store = SnapshotStore::open(path);
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
fn parse_positional_and_store(args: &[String], n_positional: usize) -> Result<(Vec<String>, Option<String>), String> {
    if args.len() < n_positional {
        return Err("missing argument(s)".to_string());
    }
    let positional = args[..n_positional].to_vec();
    match &args[n_positional..] {
        [] => Ok((positional, None)),
        [flag, path] if flag == "--store" => Ok((positional, Some(path.clone()))),
        rest => Err(format!("unrecognized trailing argument(s): {}", rest.join(" "))),
    }
}

fn usage_error(subcmd: &str, msg: &str) -> ExitCode {
    eprintln!("mmp-cli {subcmd}: {msg}\n");
    eprint!("{USAGE}");
    ExitCode::from(2)
}

/// Build a `Tier::Anchor` node over `seed`, backed by a `SnapshotStore` at
/// `store_path` if given, else an in-memory `MemStore` (same store type
/// `demo` uses, discarded on exit). Returns the shared handle to persist
/// through later, if a `SnapshotStore` is in play — see [`SharedSnapshot`].
fn build_node(seed: [u8; 32], store_path: Option<&str>) -> (Node, Option<Arc<Mutex<SnapshotStore>>>) {
    match store_path {
        Some(path) => {
            let shared = Arc::new(Mutex::new(SnapshotStore::open(path)));
            let mut node = Node::new(seed, Box::new(SharedSnapshot(shared.clone())), Tier::Anchor);
            restore_snapshot_state(&mut node, &shared);
            (node, Some(shared))
        }
        None => (Node::new(seed, Box::new(MemStore::new()), Tier::Anchor), None),
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
    let store = shared.lock().expect("snapshot store mutex poisoned");
    for claim in store.claims() {
        node.ingest_claim(claim.clone(), false);
    }
    for blob in store.group_blobs() {
        node.restore_group(blob);
    }
    if let Some(seen) = store.seen_index() {
        node.restore_seen_index(seen.clone());
    }
    if let Some(receipts) = store.receipt_filter() {
        node.restore_receipt_filter(receipts.clone());
    }
    if let Some(authority) = store.authority() {
        let _ = node.restore_authority(authority);
    }
    if let Some(blocklist) = store.blocklist() {
        let _ = node.restore_blocklist(blocklist);
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

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Bring a freshly restored node onto the CLI's persisted deadline timebase
/// before any command creates new bundles. `Node::new` starts at zero, while
/// the current snapshot format stores absolute millisecond deadlines; without
/// this first tick, a `send` bundle created before the drive loop's first tick
/// would expire immediately when the loop supplies the Unix timestamp.
fn prime_node_clock(node: &mut Node) {
    prime_node_clock_at(node, now_ms());
}

fn prime_node_clock_at(node: &mut Node, now_ms: u64) {
    node.tick(now_ms);
}

fn cmd_connect(args: &[String]) -> ExitCode {
    let (positional, store_path) = match parse_positional_and_store(args, 1) {
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

    let (mut node, shared) = build_node(seed, store_path.as_deref());
    prime_node_clock(&mut node);
    println!("node id: {}", hex(node.node_id().as_bytes()));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("mmp-cli connect: failed to start async runtime: {e}");
            return ExitCode::from(2);
        }
    };
    rt.block_on(drive_loop(&mut node, shared.as_ref(), None))
}

fn cmd_send(args: &[String]) -> ExitCode {
    let (positional, store_path) = match parse_positional_and_store(args, 3) {
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

    let (mut node, shared) = build_node(seed, store_path.as_deref());
    prime_node_clock(&mut node);
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
        Err(e) => {
            eprintln!("mmp-cli send: failed to start async runtime: {e}");
            return ExitCode::from(2);
        }
    };
    rt.block_on(drive_loop(&mut node, shared.as_ref(), Some(msg_id)))
}

fn cmd_listen(args: &[String]) -> ExitCode {
    let (positional, store_path) = match parse_positional_and_store(args, 1) {
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

    let (mut node, shared) = build_node(seed, store_path.as_deref());
    prime_node_clock(&mut node);
    println!("node id: {}", hex(node.node_id().as_bytes()));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("mmp-cli listen: failed to start async runtime: {e}");
            return ExitCode::from(2);
        }
    };
    rt.block_on(drive_loop_peripheral(&mut node, shared.as_ref()))
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
async fn drive_loop(node: &mut Node, shared: Option<&Arc<Mutex<SnapshotStore>>>, watch: Option<Id16>) -> ExitCode {
    let central = match BleCentral::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mmp-cli: no usable BLE adapter: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = central.start_scan().await {
        eprintln!("mmp-cli: failed to start BLE scan: {e}");
        central.shutdown_all().await;
        return ExitCode::from(2);
    }
    println!("scanning for MMP peers... (Ctrl-C to stop)");

    let mut events = central.events();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut dialed: HashSet<PeripheralId> = HashSet::new();
    let start = std::time::Instant::now();

    let mut frames_sent = 0u64;
    let mut messages_received = 0u64;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                node.tick(now_ms());
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
                    None => break,
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
        if node.close_actions_dropped() != 0 || central.control_events_dropped() != 0 {
            eprintln!(
                "mmp-cli: transport lifecycle overload (node close drops: {}, transport control drops: {}); shutting down",
                node.close_actions_dropped(),
                central.control_events_dropped()
            );
            central.shutdown_all().await;
            finish(shared, node, frames_sent, messages_received);
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
                    finish(shared, node, frames_sent, messages_received);
                    return ExitCode::SUCCESS;
                }
                _ => {}
            }
        }

        if watch.is_some() && start.elapsed() >= SEND_TIMEOUT {
            println!("timed out after {}s waiting for delivery", SEND_TIMEOUT.as_secs());
            central.shutdown_all().await;
            finish(shared, node, frames_sent, messages_received);
            return ExitCode::from(1);
        }
    }

    let overload = node.close_actions_dropped() != 0 || central.control_events_dropped() != 0;
    central.shutdown_all().await;
    finish(shared, node, frames_sent, messages_received);
    if overload { ExitCode::from(2) } else { ExitCode::SUCCESS }
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
async fn drive_loop_peripheral(node: &mut Node, shared: Option<&Arc<Mutex<SnapshotStore>>>) -> ExitCode {
    let peripheral = match PeripheralRole::new().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("mmp-cli: no usable BLE peripheral backend: {e}");
            return ExitCode::from(2);
        }
    };

    let mut last_advertised_ms = now_ms();
    if let Err(e) = peripheral.advertise(node.beacon(last_advertised_ms).to_vec()).await {
        eprintln!("mmp-cli: failed to start advertising: {e}");
        peripheral.shutdown_all().await;
        return ExitCode::from(2);
    }
    println!("advertising for MMP peers... (Ctrl-C to stop)");

    let mut events = peripheral.events();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    let mut frames_sent = 0u64;
    let mut messages_received = 0u64;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let now = now_ms();
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
                    None => break,
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
        if node.close_actions_dropped() != 0 || peripheral.control_events_dropped() != 0 {
            eprintln!(
                "mmp-cli: transport lifecycle overload (node close drops: {}, transport control drops: {}); shutting down",
                node.close_actions_dropped(),
                peripheral.control_events_dropped()
            );
            peripheral.shutdown_all().await;
            finish(shared, node, frames_sent, messages_received);
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

    let overload = node.close_actions_dropped() != 0 || peripheral.control_events_dropped() != 0;
    peripheral.shutdown_all().await;
    finish(shared, node, frames_sent, messages_received);
    if overload { ExitCode::from(2) } else { ExitCode::SUCCESS }
}

/// Exit path shared by every `drive_loop` return: persist to disk if a
/// `--store` path is in play, then print the run summary.
fn finish(shared: Option<&Arc<Mutex<SnapshotStore>>>, node: &Node, frames_sent: u64, messages_received: u64) {
    if let Some(shared) = shared {
        match persist_node_state(shared, node) {
            Ok(()) => println!("store persisted to disk"),
            Err(e) => eprintln!("mmp-cli: failed to persist store: {e}"),
        }
    }
    println!("--- summary ---");
    println!("frames sent to links:  {frames_sent}");
    println!("messages received:     {messages_received}");
    println!("store_len:             {}", node.store_len());
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmp_types::bundle::class;
    use mmp_types::ids::Key32;

    fn temp_snapshot_path() -> std::path::PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        env::temp_dir().join(format!("mmp-cli-snapshot-{}-{nonce}.bin", std::process::id()))
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

        prime_node_clock_at(&mut node, 1_000_000);
        node.send_text(&peer.node_id(), "clock-safe").expect("peer claims are present");
        assert!(node.store_len() > 0);
        node.tick(1_000_001);
        assert!(node.store_len() > 0, "the first drive tick must not expire a freshly queued bundle");
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

        let (mut node, shared) = build_node(seed, Some(&path_string));
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

        let (mut restored, reopened) = build_node(seed, Some(&path_string));
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
