# State machines

Status: normative MMP/1 source

## Node lifecycle

`Created -> Ready -> Discovering -> Linking -> Session -> Ready`. A node may enter `Stopping` from any state. A link failure returns to `Ready` after clearing per-session queues. Store, claim, receipt, and identity state survive a process restart according to the persistence rules in [`synchronization.md`](synchronization.md) and [`cryptography.md`](cryptography.md).

## Session lifecycle

```text
Idle
  -> HelloSent / HelloReceived
  -> NoiseHandshake
  -> Bound (encrypted ID_PROOF verified)
  -> CreditExchange
  -> Summary
  -> Reconcile
  -> Transfer
  -> Closing
  -> Closed
```

The implementation uses monotonic timers: handshake budget 5 s, idle timeout 10 s, continuous airtime cap 60 s, redial suppression 60 s, and exponential backoff from 1 s to 5 minutes. A profile MAY choose stricter values.

At every transition a receiver validates frame length, phase, and authenticated context. A frame that is valid in another phase is rejected. `ERROR` carries a bounded code and empty detail in the current implementation; the sender then emits `CLOSE`. Inbound `ERROR` and `CLOSE` terminate the session without further transfer.

## File and symbol state

File transfer is an extension. Its fetch state machine is `Unknown -> ManifestVerified -> Requested -> Receiving -> Complete` or `Abandoned`. An assembly expires after seven days of local interest. A symbol enters storage only after origin signature, symbol metadata, Reed-Solomon bounds, and Merkle proof verification.

## Group state

Group sender keys advance per member and epoch. Membership changes require a new epoch, but the current implementation still accepts some future-epoch distributions without a final authenticated administrator anchor. Implementations MUST keep this state behind the extension boundary and MUST NOT claim complete group-control security until the open decision is resolved.
