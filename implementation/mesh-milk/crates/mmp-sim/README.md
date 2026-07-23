# mmp-sim

Discrete-event mesh simulator (OQ-4/OQ-5, doc 12 §8): the protocol-truth instrument
for this project. Drives real `mmp-node::Node` instances over `mmp-tal::MemLink`
(never a mocked protocol path — the same engines a real radio would drive), plus an
`Adversary` raw-frame wrapper for the adversarial suite (replay/flood/quota-abuse/
eclipse/bloom-poison) and scheduled-meeting scenarios for deployment-profile drills.

**Layering:** depends on `mmp-types`, `mmp-core`, `mmp-node`, `mmp-tal` (`std`
feature), and `mmp-crypto` directly (only the `Adversary` wrapper needs to drive a
Noise handshake and Ed25519-sign raw frames, bypassing `Node`).

See `docs/IMPLEMENTATION_HANDBOOK.md` §4.8/§4.12/§4.13.
