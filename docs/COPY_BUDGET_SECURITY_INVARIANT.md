# Copy-Budget Security Invariant

Status: implementation/security clarification; the frozen routing specification
is unchanged.

## Correct claim

Binary Spray-and-Wait conserves the copy budget only along an honest local spray
tree. The receiver-side `L_MAX` clamp and `OriginBuckets` are not a globally
spend-once capability: a Byzantine relay can present the same inflated grant in
independent authenticated sessions and cause multiple honest receivers to admit
copies.

The production invariant is therefore:

> Every conforming receiver locally bounds admitted bytes and copy-budget values
> for each origin, even when a remote relay supplies a malicious grant. Global
> Byzantine copy conservation is not guaranteed by MMP/1.

This is a security-honesty correction, not a routing rewrite and not a claim
that the honest spray tree is unbounded. Enforcing global conservation would
require a transferable spend-once capability or distributed coordination that
is incompatible with the current disconnected, store-carry-forward design.

## Evidence

`mmp-sim::tests_adversarial::byzantine_double_grant_is_bounded_per_receiver_not_globally`
reuses one oversized grant against two independent honest receivers. Each
receiver admits only its local origin-bucket burst; the aggregate is explicitly
not asserted to be globally conserved. The ordinary router test
`spray_budget_is_conserved_over_arbitrary_trees` remains valid for honest
binary splits and must not be read as a Byzantine-capability proof.

The implementation handbook contains older wording that says the sum of budgets
across all copies never exceeds `L`. Treat this document and the proposal audit
as the current security interpretation until that stale prose is selectively
updated in a reviewed documentation pass. No specification file was edited.
