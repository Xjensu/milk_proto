# MMP extensions

These mechanisms use the MMP/1 bundle and session primitives but are outside the minimum direct-message milestone. An implementation MAY omit them while remaining an MMP/1 core implementation. Extension status is implementation-specific and is recorded in the implementation-state report.

- [`files.md`](files.md): manifests, erasure coding, Merkle proofs, and the confidentiality requirement.
- [`groups.md`](groups.md): sender keys, epochs, membership, and unresolved control authorization.
- [`friendship.md`](friendship.md): patron buffering, offers, acknowledgements, and proxy summaries.
- [`authority-announcements.md`](authority-announcements.md): scoped certificates and class-5 announcements.

Extensions MUST NOT allocate undocumented core wire values. A future extension that changes a canonical encoding requires a versioned registry entry, updated vectors, an interoperability note, and a security review.
