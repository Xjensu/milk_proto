# Authority arithmetic hardening handoff

## Scope

This continuation hardens two local monotonic-time arithmetic paths without
changing the frozen MMP/1 wire contract:

- mmp-node::AuthorityBuckets now uses saturating multiplication and addition
  when refilling an authority-subject admission bucket.
- NodeCore::try_store_own now uses a saturating deadline calculation for
  bundles created at a clock value near u64::MAX.

## Why this was required

Both paths consume host-supplied monotonic time. A faulty clock provider or a
test/fuzz input near the integer limit must not wrap a refill or expiry
deadline. In a release build, wrapping could produce incorrect admission
accounting or an already-expired/long-lived local bundle. The router origin
bucket already used saturating arithmetic; the authority bucket now follows
the same local-policy rule.

## Evidence

Regression coverage:

    cargo test -p mmp-node authority_bucket --lib -- --test-threads=1
    cargo test -p mmp-node own_bundle_deadline_saturates_at_clock_max --lib
    cargo clippy -p mmp-node --all-targets -- -D warnings

The tests cover a u64::MAX clock jump, capacity saturation, and an own bundle
whose stored and expiry timestamps remain stable at the maximum representable
value.

## Boundaries and remaining work

This is local arithmetic hardening only. It does not solve the larger E7
producer-capacity problem: `try_store_own` is per-bundle, while a fixed-capacity
provider still requires a reservation/commit design before multi-object
own-origin side effects can be made transactional. Friendship and avatar_root claim layouts also remain
implementation-provisional until a reviewed erratum or migration decision
defines their exact wire bytes.

No specification file was changed.

## Adversarial review

Potential flaw: saturation can hide an invalid clock provider.

Resolution: saturation is fail-safe for local expiry/refill arithmetic;
platform hosts must still reject clock regressions and report provider faults
at their boundary.

Potential flaw: a saturated deadline may retain a bundle longer than a real
clock would.

Resolution: this only occurs at the representable integer boundary. It
prevents wraparound; normal monotonic samples retain the existing lifetime
semantics and are covered by the existing node tests.
