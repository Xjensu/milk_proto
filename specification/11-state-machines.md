# 11 — State Machine Specification

**mesh_milk node behavior.** Status: Phase 4 draft; **revised 2026-06-11 post-audit** (`AUDIT.md`, `CHANGELOG.md`). Defines the normative FSMs an implementation must realize; doc 12 maps them onto Rust types (typestate where practical). Timers table in §7. All transitions not listed are protocol errors → the FSM's designated error state.

> **Audit revision summary (this doc):** §4 bundle FSM drops the obsolete key-unknown QUARANTINE path (origin pubkey is now inline → verification unconditional, AUDIT F2); I2 adds the receive-side budget clamp (AUDIT F3); I5 adds SeenIndex-from-first-sight (AUDIT F1).

---

## 1. Node lifecycle FSM

```
            ┌────────┐ keys loaded ┌──────┐
 BOOT ─────▶│  INIT  │────────────▶│ IDLE │◀────────────────┐
            └────────┘             └──┬───┘                 │
              │ no keys: generate     │ scheduler tick      │ all sessions closed
              ▼                       ▼                     │
            FIRST_RUN          ┌────────────┐   scan hit  ┌─┴────────┐
            (identity gen,     │ DISCOVERING│────────────▶│ ENGAGED  │ (≥1 session FSM live)
             claims publish)   └────────────┘  & dial ok  └──────────┘
                                      ▲   │ duty timer
                                      └───┘ (slot on/off)
```

- IDLE: radios duty-cycled per doc 04 §5; beacon TX continues in all states except SUSPENDED.
- ENGAGED admits up to `MAX_SESSIONS` (T2: 2, T3: 8, Core: 1 ⚠) concurrent session FSMs.
- **SUSPENDED** (not drawn): OS revokes radio (Doze, bg limits) → persist state, declare honest tier on resume (doc 04 §3). Every durable structure is crash-consistent: resume == reboot.

## 2. Tier ladder FSM (doc 04 §3, §5)

States T0–T3; transitions driven by (battery %, charger, OS mode, user setting):

```
T2→T1: battery <20% | FG service lost | iOS backgrounded
T1→T2: battery >25% (hysteresis +5%) & FG capability & user opt-in
T2→T3-like: charging & user opt-in        T3→T2: unplugged
any→T0: never (T0 is a build profile, not a runtime state)
```

Tier changes take effect at the next beacon rotation (no mid-session downgrades; in-flight sessions complete under admission rules current at HELLO).

## 3. Session FSM (one per peer connection; consumes doc 10 frames)

```
 DIALING ──connect──▶ HANDSHAKE ──HS1..HS3 ok──▶ BINDING ──ID_PROOF ok──▶ ESTABLISHED
    │ timeout/refuse        │ bad MAC / timeout        │ bad proof              │
    ▼                       ▼                          ▼                        ▼
 BACKOFF ◀────────────── ABORT ◀──────────────────── ABORT              ┌── SUMMARY
                                                                        │      ▼
 CLOSING ◀── CLOSE/airtime cap/idle ── BLOB ◀── TRANSFER ◀── RECONCILE ◀┘
    │                                   ▲ ▲        │ ▲
    ▼                                   └─┴────────┘ │   (phases may interleave per
 CLOSED                                   WANT-driven┘    domain; order per doc 03 §5.2)
```

Rules:
- HANDSHAKE+BINDING budget: 5 s ⚠ total; failure → BACKOFF with exponential delay (1 s base, ×2, cap 5 min ⚠, per-peer).
- Phase order is per *domain* (doc 06 §2): D0 SUMMARY/RECONCILE/TRANSFER completes before D1 begins, etc.; BLOB only after D0–D3 or on explicit WANT.
- Any frame violating phase grammar → ERROR + CLOSING (no tolerant parsing — doc 10 §6).
- CLOSING flushes CREDIT-owed frames, sends CLOSE, disconnects; peer that receives CLOSE stops sending immediately.
- Crash/disconnect at any point is equivalent to CLOSING without flush: all phases idempotent (doc 03 §5.2).
- Inbound sessions: same FSM from HANDSHAKE (responder); admission check (quota, blocklist, busy) happens before HS response — silent disconnect on deny (no oracle for blocked parties, doc 07 §6.5).

## 4. Bundle lifecycle FSM (per bundle, per node)

```
 RECEIVED ──sig ok (key inline)──▶ STORED ──contact──▶ OFFERED ──accepted──▶ HANDED_OFF (budget split)
    │ sig fail / non-canonical /        ▲ │ receipt seen (exact D0)        │ budget=1 & focus
    │ origin→NodeID inconsistent        │ ▼                               ▼
    ▼                                   │ PURGED                        RELEASED
 DROPPED                                │ ▲
                                        │ │ lifetime 0 / eviction
                                        │ EXPIRED/EVICTED
 (terminal: DROPPED, PURGED, EXPIRED, EVICTED, RELEASED, DELIVERED)
 DELIVERED: dest == self → decrypt path → receipt emitted (doc 05 §7)
```

There is **no QUARANTINE state** in the bundle FSM (AUDIT F2): the origin public key is carried inline (doc 03 §4.1), so a received bundle is either verifiable-and-STORED or DROPPED on sight — the old "origin key unknown → hold → key-arrival" path is gone. (A separate, tiny transient *frame-reassembly* buffer exists at the transport layer, doc 09 §5.1 / doc 10 §6.4 — it is not this FSM and never forwards.)

Invariants (testable, doc 12):
- I1: a bundle reaches STORED only after successful canonical-S signature verification against its inline origin key **and** a consistent `NodeID = BLAKE3(origin)[0..16]` check (doc 03 §4.1.2); failure → DROPPED, never OFFERED.
- I2: STORED→OFFERED requires admission re-check (receipt filter, blocklist may have changed); any copy budget adopted from a peer is **clamped to L_MAX on receive** (doc 03 §4.5, doc 05 §2.3.1, AUDIT F3). **Rev. B second admission gate (doc 18 §5.2):** for class-5 (announcement) bundles, the declared `priority` byte is additionally clamped to 2 (NORMAL) on receive unless the bundle's `origin` matches a verified, unexpired, budget-remaining AuthCert.subject for the receiver-subscribed scope — a receiver-local admission/rendering decision, never a wire mutation (declared priority stays intact for RBSR partitioning, doc 18 §5.2 CR2bis-3).
- I3: lifetime decremented before any hand-off; clamp on receive (doc 03 §4.3). Hand-off intent (bundle ID, granted budget) is persisted *before* the BUNDLE frame is sent; crash recovery treats unconfirmed hand-offs as completed — under-spray, never over-spray (doc 14 F15 review).
- I4: own-originated bundles never EVICTED before EXPIRED (doc 04 §2).
- I5: a BundleID enters SeenIndex on first sight and persists for class-max lifetime regardless of later `lifetime_ms` values (resurrection resistance, doc 03 §4.2, AUDIT F1); PURGED requires an exact authenticated D0 receipt, never a Bloom hit (doc 06 §6.3, AUDIT F11).

## 5. Friendship FSM (doc 04 §4, both roles)

```
 NONE ─OFFER sent/recv─▶ PENDING ─ACK+counter-sig─▶ ACTIVE ─renewal claim─▶ ACTIVE
   ▲                        │ timeout 60 s ⚠           │ expiry 24 h ⚠ / revoke claim
   └────────────────────────┴──────────◀───────────────┘ → NONE (buffered bundles → normal spray pool)
```

ACTIVE (friend side): buffer dependent's traffic (quota slice 25% ⚠ of own), answer proxy RECONCILE (doc 06 §7), resolve ephem_ids for dependent. ACTIVE (dependent side): poll schedule per tier (T0: 5 min ⚠; T1: opportunistic). Friendship state is a counter-signed claim pair — survives reboots, gossips like any claim (doc 10 §4.4).

## 6. Blob assembly FSM (per root, requester side)

```
 WANTED ─claim/root verified─▶ FETCHING ─k symbols/block─▶ BLOCK_DONE* ─all blocks─▶ ASSEMBLED
    │ root claim invalid          │ symbol fails Merkle path → drop symbol, blacklist sender count++
    ▼                             │ lifetime of interest 7 d ⚠ / user cancel
 REJECTED                         ▼
                               ABANDONED (partial symbols retained until expiry — may serve others)
```

Partial-state note: a node MAY serve symbols from ABANDONED/FETCHING states (it verified each symbol individually) — partial caches accelerate the swarm without integrity risk (doc 08 §8).

## 7. Timer table (consolidated; all ⚠ pending doc 13)

| Timer | Value | FSM | Purpose |
|---|---|---|---|
| beacon interval | 2–10 s adaptive | node | presence |
| scan slot | 30 ms / 1.28 s | node | discovery duty |
| burst scan | 30 s, ≤6/min | node | contact exploitation |
| ephem/RPA rotation | 15 min | node | unlinkability |
| handshake budget | 5 s | session | stall kill |
| session idle | 10 s | session | park/close |
| session airtime cap | 60 s | session | fairness |
| redial suppress | 60 s | scheduler | thrash control |
| backoff base/cap | 1 s / 5 min | session | failure damping |
| friendship expiry / poll | 24 h / 5 min (T0) | friendship | liveness |
| reassembly stall | session idle (doc 09 §5.1) | transport | discard partial frame (AUDIT F10; replaces the obsolete quarantine timer) |
| receipt bucket | 6 h × 12 | store | filter rotation |
| claim refresh (own) | 24 h | services | liveness of presence claims |
| AuthCert `priority_budget` refill | per-cert field, admissions/hour ⚠ (doc 18 §2, §5.3) | bundle (admission) | per-authority elevated-priority rate limit, keyed on AuthCert.subject |
| `not_after` clock-skew tolerance | 15 min ⚠ (doc 18 §6, CR-A6) | bundle (admission) | AuthCert/revocation expiry check against receiver's own local clock (no shared clock, doc 03 §1) |
| REVOCATION_LIST republish | ordinary claim-gossip cadence (doc 06 §4.3) — no dedicated timer; re-offered whenever `seq` differs in the SUMMARY claim digest | claims | issuer's revocation state (doc 18 §6) propagates at gossip speed, capped at `REVLIST_MAX = 48` ⚠ entries per update |

## 8. Persona review checkpoints

- **Distributed Systems Reviewer — Potential flaw:** "Phase-locked domains (§3) serialize: a huge D3 reconciliation starves BLOB even when the user is waiting on an avatar." *Resolution:* D-phases carry a per-domain airtime sub-budget (D3 ≤ 40% ⚠ of session cap); WANT-driven BLOB pre-emption allowed after D0/D1 complete (user-visible traffic first, relay charity second). FSM edge added (TRANSFER⇄BLOB interleave). Accepted.
- **Skeptical Professor — Potential flaw:** "Does §6 letting partial blob caches serve symbols conflict with verify-before-store (I1)?" *Resolution:* no — symbols in §6 are *individually verified* against a signed Merkle root before storage (they're never unverified), exactly as bundles are verified against their inline origin key (I1). Post-audit, with QUARANTINE removed (AUDIT F2), the rule is uniform: nothing is ever stored or served before its own signature/Merkle verification. Distinction documented here to prevent implementer confusion. Accepted.
- **Battery Specialist:** SUSPENDED→resume on Android Doze cycles could thrash persist/restore. *Resolution:* persistence is incremental (dirty-page style, doc 12); resume cost target <50 ms / <1 J ⚠; measured in Phase 5. Accepted provisionally.
- **Security Auditor:** silent-deny on inbound admission (§3) must be indistinguishable from radio loss to the denied peer — verified: no ERROR frame, plain disconnect, same timing as timeout path (constant-time-ish deny). Accepted.
- **Performance Engineer:** per-bundle FSM state (§4) must not materialize as per-bundle allocations — state is derivable from store indices + session context; doc 12 records this as an implementation constraint (enum-in-index, not object-per-bundle). Accepted.

## 9. Cross-references

Frames each FSM consumes → doc 10. Rust typestate realization + invariant tests I1–I4 → doc 12. Timer/energy interaction → doc 13. Failure-mode walks of these FSMs (partition, storm, clock anomalies) → doc 14. Blob-plane symbol/manifest schema → doc 16. Deployment-profile timer presets (train/campus) → doc 17. Rev. B class-5 admission gate (I2 above), AuthCert/revocation timers (§7) → doc 18 §5.2/§6.
