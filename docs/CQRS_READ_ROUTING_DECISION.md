# CQRS and read-routing decision

Date: 2026-09-12  
Status: **single-primary baseline; defer replicas and sharding**

CQRS is applied as an application responsibility boundary: Rails submits commands through typed operations, while Hanami exposes separate query and mutation actions. PostgreSQL remains the single authoritative writer and reader. Rails never chooses a database host, and no browser request is classified as replica-safe merely because it uses `GET`.

Consistency-sensitive reads stay on the primary. This includes authorization, account/session state, workload assignment, schedule placement and the immediate read-back after a mutation. Reporting and public schedule queries may become replica candidates later, but only after their tolerated staleness, cache invalidation and authorization scope are written down.

No read replica, shard, sticky-session marker or automatic write replay is enabled in the local stack. Adding those without a measured lag budget would make a successful schedule save appear to disappear, while replaying an uncertain write could duplicate a lesson. The current rollback route is therefore simple: all reads and writes use the primary database.

Before introducing a replica, implement and measure one explicit read policy in the API: primary for mutations and mutation-dependent reads, optional replica for a documented reporting query, bounded fallback to primary when lag or health is outside limits, and pool recycling after promotion. After a successful write, use a server-trusted consistency marker (preferably a WAL/LSN gate tied to the current timeline) rather than a browser-supplied timestamp. Verify write/read-back, delayed replica, stale cache, authorization change, replica loss and promotion before enabling any route.

True sharding remains deferred until representative scale measurements justify partitioning and a design exists for cross-group queries, authorization, schedule conflict checks and transaction boundaries. A master–slave label alone is not a failover design; promotion requires fencing so two writers cannot accept schedule mutations simultaneously.
