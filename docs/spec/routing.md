# Routing, quotas, and copy budgets

Status: normative MMP/1 source

MMP uses delay-tolerant store-carry-forward routing. A node may forward a bundle when the destination is present, when a locally observed utility estimate is sufficient, or when a profile explicitly permits a controlled relay. Utility is based on local contact observations; unauthenticated third-party utility claims MUST NOT change forwarding policy.

The default routing strategy is binary Spray-and-Wait followed by Focus forwarding. In the spray phase a sender splits a local copy budget. In the focus phase one copy is forwarded when the candidate utility exceeds the local threshold. The receiver clamps any received grant to `L_MAX = 32`, and class-0 receipts use a smaller local ceiling of eight.

The conservation statement is deliberately narrow: an honest binary-split tree preserves its budget locally. Receiver-local clamps, per-origin token buckets, and per-destination in-flight caps limit admission. A Byzantine relay can replay or present the same grant to multiple receivers; MMP/1 does not provide globally spend-once copy conservation.

The standard per-origin bucket is 32 KiB/hour with a 128 KiB burst. Web-of-Trust policy may multiply those local values by four. The standard node quota is 128 MiB, the Core quota is 32 KiB, and the Anchor quota is 2 GiB. These are local admission policies; they do not alter bundle encoding.

Utility estimates use an initial probability of 0.5, per-hour decay of 0.98, a spray threshold of 0.30, a focus threshold of 0.15, and a tie margin of 0.5 in the reference implementation. A deployment MAY tune policy values after measurement. It MUST report them as profile parameters and MUST NOT present them as delivery guarantees.

`P_TABLE_CAP = 256`, `CONTACT_GRAPH_CAP = 4096`, and `IN_FLIGHT_CAP = 64` bound attacker-influenced state. Counters saturate rather than wrap. A node evicts only entries allowed by the policy; it MUST NOT evict a verified object merely to admit an unverified hint.

Research mechanisms such as evidence-weighted spray, deadline scheduling,
mobility vouchers, and DoF/fountain coding remain outside MMP/1. They do not
change MMP/1 routing behavior or wire allocation.
