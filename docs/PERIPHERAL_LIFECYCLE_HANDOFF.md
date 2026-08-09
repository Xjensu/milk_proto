# Peripheral lifecycle and BlueZ evidence handoff

Status: host-testable lifecycle coverage only. The frozen `specification/`
tree and all wire behavior are unchanged.

## Concrete regression added

The peripheral ingress already fenced stale reader callbacks after a
same-link replacement, but had no equivalent regression for the notify/write
direction. `event_queue::tests::stale_writer_data_is_rejected_after_replacement`
now exercises the direction-specific writer stamp:

1. admit writer generation 1;
2. queue a generation-1 write;
3. install generation 2 and preserve the ordered Down→Up lifecycle;
4. assert generation-1 data is rejected as stale;
5. assert generation-2 data is delivered after the replacement Up.

This is a pure bounded-ingress test. It does not construct a BlueZ object,
open D-Bus, send an ATT packet, or claim physical reconnect behavior. The
reader and writer paths now have symmetric host-side stale-endpoint coverage.

Focused verification:

```text
cargo test -p mmp-tal-peripheral stale_writer_data_is_rejected_after_replacement -- --test-threads=1
cargo test -p mmp-tal-peripheral --all-targets -- --test-threads=1
cargo clippy -p mmp-tal-peripheral --all-targets --all-features -- -D warnings
```

## Remaining hardware-only gate

The Linux backend (`src/linux.rs`) still requires a real BlueZ daemon and
adapter to prove the following lifecycle sequence:

```text
serve GATT application → advertise → central subscribes/writes
→ negotiated AcquireWrite/AcquireNotify MTU is observed
→ bidirectional chunks reach the peer
→ disconnect or failed notify emits one LinkDown
→ delayed callbacks cannot affect a replacement
→ shutdown_all unregisters advertisement/application and closes endpoints
→ a fresh process can advertise and reconnect
```

The host tests cover pure chunk arithmetic, generation fencing, bounded gate
waits, event-lane saturation, and stale control/data behavior. They cannot
prove BlueZ D-Bus registration, adapter power state, socket EOF semantics,
real ATT MTU negotiation, notification subscription order, or two-machine
PC/mobile interoperability. Those remain user-executed Linux/BlueZ gates
described in `docs/MANUAL_INTEROP_TESTING.md`; no local result should be
presented as physical BLE evidence.

## Review boundary

- No production scheduler, routing, or protocol fields were changed.
- No dirty `mmp-types` files were touched.
- No `specification/` file was touched.
- A successful host test proves only the writer-generation invariant, not a
  performance, battery, or reconnect guarantee.
