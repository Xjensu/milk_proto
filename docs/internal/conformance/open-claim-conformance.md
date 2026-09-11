# Claim conformance discrepancies

The Friendship body and avatar-root claim body are not identical across the former specification text and current Rust implementation. Current Rust validates exchange keys, prekeys, the current Friendship countersignature, group epoch-anchor structure, and revocation-owner binding. Unknown fields remain opaque.

This document is a blocker for independent claim interoperability. It does not authorize a silent wire change. A reviewed erratum or migration design must select one grammar, then update the vector catalogue and every implementation.
