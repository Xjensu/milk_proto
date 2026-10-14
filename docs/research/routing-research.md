# Routing research

Spray-and-Wait gives a bounded replication baseline. Focus forwarding adds a local utility decision after the spray phase. Utility values are observations, not authenticated claims, and a local active adversary can poison them. Receiver-side budgets, per-origin buckets, quotas, and receipts limit the impact of a bad observation.

MMP/1 research explores evidence-weighted spray, deadline-aware scheduling, mobility vouchers, and fountain-style degrees-of-freedom transfer. Each mechanism depends on calibrated probabilities and a defined adversary. Correlated human mobility invalidates independent-delivery arithmetic unless trace evidence supports it. Vouchers authenticate an issuer statement, not a future encounter. Fountain coding changes CPU, RAM, authentication, and intellectual-property trade-offs. These mechanisms remain experiments and do not modify MMP/1.

The implementation currently provides binary copy splitting, Focus thresholds, bounded utility tables, and local origin admission. Priority partitions and authority-weighted admission remain open decisions.
