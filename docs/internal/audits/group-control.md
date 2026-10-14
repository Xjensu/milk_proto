# Group control audit

## Finding

Sender-key encryption and per-epoch ratchets are implemented, but the current pairwise distribution path does not authenticate an administrator decision for every future epoch. Late-join and partition-merge behavior is not fully specified.

## Impact

A receiver can hold remote future-epoch state without a final authenticated floor. A removed member or an attacker who can inject a validly signed member distribution may create ambiguity about which epoch is current. Historical sender-key confidentiality also depends on local epoch retention.

## Current behavior

The implementation preserves future-epoch acceptance so legitimate post-removal convergence is not broken. Candidate group-control types are quarantined and are not normative wire semantics.

## Required resolution

Define an administrator anchor, epoch floor, partition merge rule, late-join key set, replay policy, and bounded retention. Add positive and negative vectors before promoting the extension.
