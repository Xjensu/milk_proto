# Authority and announcement extension

Authority certificates and official announcements are an optional class-5 extension. A receiver may pin an operator root out of band or learn a root through ordinary claim gossip. The two trust states MUST be shown separately to users. Gossip does not repair the first-contact trust problem.

The current certificate layout is 152 bytes:

```text
suite:1, subject:32, scope:16, capabilities:1,
priority_budget:2, not_after:4, issuer:32, signature:64
```

`not_after` is checked with the receiver's local wall clock and a 15-minute skew allowance. Bundle aging remains monotonic. A class-5 announcement is signed by its issuer and admitted at the receiver only when the scope, capability, expiry, and local token bucket permit it. The receiver clamps declared priority to normal when no valid authority chain authorizes a higher class.

The extension does not create global priority or global rate enforcement. A compromised issuer is bounded by its remaining local budget and scope; a compromised operator root can continue issuing within its pinned scope until operators re-pin. RBSR priority partitions, gossiped-root learning, and full revocation semantics remain open. Authority support MUST NOT block direct MMP/1 implementation.
