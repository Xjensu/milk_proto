# Runtime and platform roadmap

Date: 2026-09-06. Status: accepted user requirements; implementation and experiments remain pending unless explicitly marked otherwise. The primary delivery remains the complete Rails migration.

## Concurrency: Fibers for I/O, Ractors for computation

Use scheduler-aware I/O for HTTP, Redis, file reads/writes and waiting within request handling. Fibers provide cooperative concurrency only when the scheduler and client operation support it. Inventory blocking libraries, filesystem calls and database pool behavior before switching the API server. Rails already has Falcon/async-http/async-redis; the API still uses Puma and synchronous clients in some paths. [Ruby Fiber documentation](https://docs.ruby-lang.org/en/3.4/Fiber.html).

Run CPU-heavy recommendation scoring and file-content calculations in a bounded Ractor pool inside durable background workers. Transfer immutable value data; keep Rails containers, repositories, database connections and request objects outside Ractors. Benchmark Ruby/native-library compatibility and transfer costs. Existing `lib/api_geteway/media/sanitizers/entropy_scanner.rb` is a starting point. File processing mixes I/O and computation: divide these phases rather than treating every background task as CPU-bound. Ractors do not persist or schedule jobs across process restarts. [Ruby 3.4 Ractor documentation](https://docs.ruby-lang.org/en/3.4/Ractor.html).

Evidence required: concurrent delayed-I/O handling, bounded memory/queue depth, deterministic computation, worker restart recovery and measured throughput for the selected workload.

## Circuit breaker

Extend the existing Rails `Resilience::Call`/`Breaker`, starting with auth coverage. Bound call time, count transport/timeouts/5xx failures, open after the configured threshold, and fail immediately while open. Return Russian service-unavailable messages; stale data is acceptable only for explicitly cacheable public reads and must be visibly marked. Never cache auth responses or retry token rotation automatically.

Follow-up: atomic transitions and one half-open recovery probe across Falcon processes; distinguish operation-specific errors from whole-service outages; prevent a late success from erasing a newer outage; bounded behavior when Redis itself fails; observable transitions. Do not treat validation/authorization rejections as infrastructure failures. Keep retry budgets bounded and use idempotency keys before retrying non-idempotent commands.

## Logging, Grafana and audit

Target pipeline: service stdout JSON -> Fluentd -> Loki -> Grafana. Metrics: Prometheus -> Grafana, with alerts for availability, upstream latency/error rate, breaker state, Outbox age, worker failures, replication lag and disk/buffer exhaustion. Fluentd is the selected collector for this Compose deployment; retain Logplex as an alternative to evaluate only if deployment requirements justify it. Grafana's Loki integration supports Fluentd through `fluent-plugin-grafana-loki`. [Loki Fluentd integration](https://grafana.com/docs/loki/latest/send-data/fluentd/).

Implementation steps:

1. Define a versioned JSON schema: timestamp, service, environment, severity, event, request/trace ID, route template, method, status, duration and safe error code. Avoid raw query strings, request bodies, cookies, tokens, passwords and arbitrary exception messages. Configure the Rails formatter as well as subscribers: tagged plain text surrounding JSON is not a clean JSON stream.
2. Add a separate Compose observability overlay, pinned compatible images, persistent bounded storage, retention and provisioned data sources/dashboards. Keep application availability independent of collector availability; configure bounded buffers/retry and observable dropped-log counts. Avoid access to the Docker control socket.
3. Keep internal collector/Loki/Prometheus endpoints unexposed; bind local Grafana access to loopback, use supplied secrets and authentication, and document TLS for external access. Grafana uses a separate host port because Rails already uses 3000.
4. Distinguish operational logs from the existing API `audit_logs`. Audit actor, scope, action, entity ID, safe change summary, outcome and correlation ID. Review existing event listeners for atomicity and coverage. Protect reads by role, restrict update/delete privileges, and define retention/export. Log failures must not silently erase the durable audit record of a committed mutation.
5. Validate one real request end to end, rejection/redaction, collector outage/recovery, retention and an alert reaching its configured destination. Do not call the stack deployed until these checks run.

## Transactional Outbox and batch processing

An Outbox migration, repository and relay already exist. Inspect all producers to prove the business write and event insert share one database transaction. The current relay calls Redis `PUBLISH`; that call alone does not prove a consumer received or durably processed the event. Before using it for critical recommendation/index updates, choose a durable transport with acknowledgements, replay and deduplication. Preserve existing Avro consumers until a compatible transition exists. [Transactional Outbox pattern](https://microservices.io/patterns/data/transactional-outbox).

Use stable event IDs, aggregate versions, idempotent consumers, leased claims, retry backoff and dead-letter visibility. Publish outside the database transaction holding the claim; acknowledge only after transport acceptance. Preserve per-aggregate order where it matters. Verify crash windows between write/claim/publish/ack and duplicate delivery.

Batch processing groups bounded work to amortize SQL/network overhead. Project uses: claiming Outbox events, bulk search indexing, recomputing recommendation candidates, refreshing read projections and audit retention/export. Use keyset pagination or claimed work IDs, configurable batch size and concurrency, short transactions, checkpoints, cancellation and per-item outcomes. Never load an entire semester dataset without bounds or let one malformed item stall the entire queue. Measure throughput, tail latency, lock duration and peak memory before tuning. The completed Excel worker stays outside this work.

## CQRS, replicas, stickiness and failover

The intended acronym is CQRS. Separate command operations/repositories from read queries/projections first, retaining the API as the only database owner. This can begin on one database; CQRS does not itself require replication or sharding. [CQRS guidance](https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs).

- Commands (including POST/PATCH/PUT/DELETE) and authorization-critical reads use the writable primary. Select routing by operation semantics, not just the HTTP verb.
- Eligible read queries can use healthy replicas. Read/write replicas contain the same data; shards partition data and require a separate partition-key/transaction design. Defer sharding until measurements demonstrate the need.
- After a write, retain a trusted server-side consistency marker and route that user's relevant reads to the primary until a replica is demonstrably caught up. A TTL-only pin is a practical baseline with a documented consistency limitation; a replay-LSN gate can provide a stronger guarantee. Merely sticking to an arbitrary replica can still return stale data. Invalidate or bypass stale caches/projections too.
- Failover needs leader election, fencing against two writable primaries, routing updates, connection recycling and former-primary rejoin rules. Evaluate a supported PostgreSQL HA manager with a quorum store. Define RPO/RTO and synchronous-versus-asynchronous tradeoffs first. Compose on one host is a rehearsal, not host-level high availability. [PostgreSQL replication](https://www.postgresql.org/docs/15/warm-standby.html).

Acceptance: backup/restore rehearsal on disposable data, replica lag after writes, read-only enforcement, primary/replica loss, quorum loss, stale-primary fencing, pin reset across promotion and recovery. Never reinitialize the current PostgreSQL volume to implement this.

## Solr experiment

Evaluate Solr for Russian teacher/course/room search, typo tolerance, faceting and relevance against a PostgreSQL full-text/trigram baseline. Use representative anonymized queries and data. Keep the authoritative records in PostgreSQL; index via a durable Outbox consumer, support deletes and full reindex, enforce authorization filters before returning results. Isolate the experiment from the scheduling write path. [Solr reference guide](https://solr.apache.org/guide/solr/latest/).

Deliverable: reproducible query set, relevance results, p95 search latency, index freshness, memory/storage/operations cost and an adopt/defer decision. Installation alone is not a successful experiment.

## Graph recommendations and SPARQL

SPARQL is a query language for RDF, not a database embedded in PostgreSQL. Apache AGE supplies a PostgreSQL property graph queried with Cypher. If SPARQL is a firm interoperability requirement, evaluate an RDF store or RDF-to-relational mapping separately. [W3C SPARQL 1.1](https://www.w3.org/TR/sparql11-query/), [Apache AGE overview](https://age.apache.org/overview/).

Prototype teacher -> qualification -> discipline, group -> workload, room -> equipment, and time/availability relationships on disposable data. Compare relational SQL and an AGE graph for explainable candidate discovery. Verify extension compatibility with the existing PostgreSQL 15/PostGIS/pgRouting image before preparing an isolated build. Keep API scheduling constraints authoritative: graph traversal suggests candidates, while constraint validation enforces feasibility. A graph database does not by itself solve timetable optimization.

Deliverable: identical constraints/input data, explained recommendations, query/runtime comparison, incremental update/rebuild strategy and adopt/defer decision. No unreviewed extension installation in the live database.

## Protobuf experiment

The API already uses Avro and shared `schemas/avro`; do not introduce a second production event format without evidence. Define one versioned schedule-change envelope in an isolated Protobuf experiment and compare Ruby/Python encoding size, latency and compatibility with the Avro equivalent. Keep Rails browser communication on the existing HTTP API during evaluation.

Use stable field numbers, reserve removed numbers/names, represent presence intentionally, bound payload sizes, and verify old/new consumers. Record generated-code/runtime versions. Protobuf describes serialization; it does not supply transport, authentication, encryption, durable delivery or authorization. [Protobuf language guide](https://protobuf.dev/programming-guides/proto3/).

## Delivery order

Close current resilience/auth gaps alongside the Rails migration; then implement observability and durable event processing; run bounded search/graph/serialization experiments; finally rehearse and deploy database routing/HA with rollback and verified backups. Keep each stage's runtime evidence and independent review in the main migration ledger.
