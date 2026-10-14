# Friendship and proxy buffering extension

Friendship is a signed relationship in which a patron buffers a dependent's bundles while the dependent is offline. An offer and acknowledgement establish the relationship and its expiry. The current default relationship lifetime is 24 hours, a dependent quota is 16 MiB, and a dependent may hold up to three patrons.

The patron stores only verified bundles and keeps receipts end-to-end. A `PROXY_SUMMARY` is a hint for the dependent's inventory; it does not authorize deletion and does not let a patron forge a dependent signature. The dependent must eventually connect to complete ordinary reconciliation.

The reference implementation covers offer/acknowledgement, buffering, quota enforcement, receipts, and expiry. Offline patron reconciliation, signed proxy summaries with a validity horizon, and a complete ephemeral-ID resolution path remain deferred. These behaviors MUST NOT be described as complete in a deployment profile.
